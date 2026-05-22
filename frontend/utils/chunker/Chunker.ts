import { extractYamlFrontmatter, splitByHeadings } from "./extract";
import { extractTable, linearizeTable } from "./table";
import { countTokens, splitOversizedParagraph } from "./token";
import {
  removeMarkdownFormatting,
  removeHorizontalRules,
  removeEmptyLines,
  removeListNumbering,
  stripHeadingMarkers,
  removeListMarkers,
  removeMarkdownLinks,
} from "./formatting";
import { formatLatex } from "./latex";
import { ChunkOptions, TableInfo } from "./_types";

// ── Preset option bundles ────────────────────────────────────────────────

export const DEFAULT_OPTIONS: ChunkOptions = {
  // format
  lowerCase: true,
  removeLinks: true,
  removeFormatting: true,
  formatLatex: true,
  removeRules: true,
  removeNumbering: true,
  stripHeadingMarkers: true,
  removeListMarkers: true,
  compactLines: true,
  // rules for chunking
  chunkByHeadings: true,
  headingLevel: 2,
  includeHeadingInChunks: true,
  separateSubHeadings: false,
  keepSubheadingWithContent: true,
  // rules for tables
  preserveTables: true,
  linearizeTables: true,
  eachTableRowAsSeparateChunk: true,
  separateTablesAsChunks: false,
  // tokens
  maxTokens: 256,
  overlapTokens: 0,
};

/**
 * Structural options used for the single segmentation pass in chunkPaired().
 * Embedding and generation both use these for splitting, so chunk counts
 * always match by construction. Only cleaning/reformatting options differ
 * between the two passes.
 *
 * Cleaning flags are all set to "pass-through" (false/off) so the structural
 * pass preserves original markdown. Cleaning is applied separately per-target
 * after the split.
 */
export const STRUCTURAL_OPTIONS: Partial<ChunkOptions> = {
  // Cleaning: pass-through (no stripping during structural split)
  lowerCase: false,
  removeLinks: false,
  removeFormatting: false,
  formatLatex: false,
  removeRules: false,
  removeNumbering: false,
  stripHeadingMarkers: false,
  removeListMarkers: false,
  compactLines: false,
  // Structural split
  chunkByHeadings: true,
  headingLevel: 2,
  includeHeadingInChunks: true,
  separateSubHeadings: true,
  keepSubheadingWithContent: true,
  preserveTables: true,
  separateTablesAsChunks: true,
  linearizeTables: true,
  eachTableRowAsSeparateChunk: true,
  maxTokens: Infinity,
  overlapTokens: 0,
};

export const EMBEDDING_OPTIONS: Partial<ChunkOptions> = {
  lowerCase: true,
  removeLinks: true,
  removeFormatting: true,
  formatLatex: true,
  linearizeTables: true,
  chunkByHeadings: true,
  headingLevel: 2,
  includeHeadingInChunks: true,
  separateSubHeadings: true,
  separateTablesAsChunks: true,
  keepSubheadingWithContent: true,
  removeRules: true,
  compactLines: true,
  removeNumbering: true,
  stripHeadingMarkers: true,
  removeListMarkers: true,
  eachTableRowAsSeparateChunk: true,
};

// ── Generation preset: keep original formatting intact ────────────────
// For LLM context windows — preserves markdown markup, links, LaTeX,
// and original casing so the model sees the document as-written.
export const GENERATION_OPTIONS: Partial<ChunkOptions> = {
  // ── Cleaning (all off) — keep original markdown ─────────────────
  lowerCase: false,
  removeLinks: false,
  removeFormatting: false,
  formatLatex: false,
  removeRules: false,
  compactLines: false,
  removeNumbering: false,
  stripHeadingMarkers: false,
  removeListMarkers: false,

  // ── Splitting ───────────────────────────────────────────────────
  chunkByHeadings: true,
  headingLevel: 2,
  includeHeadingInChunks: false, // no heading prepend (markdown is self-contained)
  separateSubHeadings: true,
  keepSubheadingWithContent: false,

  // ── Tables ──────────────────────────────────────────────────────
  linearizeTables: false, // keep raw markdown tables
  separateTablesAsChunks: true,
  preserveTables: true,

  // ── Size limits ─────────────────────────────────────────────────
  maxTokens: Infinity, // no hard cap (let LLM context window decide)
  overlapTokens: 0,
};
// ── Chunker class ────────────────────────────────────────────────────────

export class Chunker {
  private options: ChunkOptions;

  constructor(options?: Partial<ChunkOptions>) {
    this.options = { ...DEFAULT_OPTIONS, ...options };
  }

  setOptions(options: Partial<ChunkOptions>): void {
    this.options = { ...this.options, ...options };
  }

  chunkForEmbedding(content: string) {
    return this.runPipeline(content, { ...EMBEDDING_OPTIONS });
  }

  chunkForGeneration(content: string) {
    return this.runPipeline(content, { ...GENERATION_OPTIONS });
  }

  /**
   * Single structural split, dual cleaning pass.
   *
   * Uses STRUCTURAL_OPTIONS for segmentation (same for both paths),
   * then cleans each raw chunk twice:
   *   - embeddingText: full cleaning (lowercase, no links, no formatting, etc.)
   *   - generationText: preserves original markdown (no cleaning)
   *
   * This guarantees chunk counts match by construction (N == N),
   * solving the asymmetry problem where SimilarityMatcher queried
   * the retrieval index with "dirty" text that didn't match the
   * indexed embeddings.
   */
  chunkPaired(content: string): {
    pairs: Array<{ embedding: string; generation: string }>;
    metadata: Record<string, unknown> | null;
  } {
    // Single structural split — same options for both paths
    const structResult = this.runPipeline(content, { ...STRUCTURAL_OPTIONS });
    const rawChunks = structResult.chunks;

    // Clean each raw chunk with embedding options → canonical form for search/dedup
    const embedOpts: ChunkOptions = {
      ...this.options,
      ...EMBEDDING_OPTIONS,
    };
    const genOpts: ChunkOptions = {
      ...this.options,
      ...GENERATION_OPTIONS,
    };

    const pairs: Array<{ embedding: string; generation: string }> = [];

    for (const raw of rawChunks) {
      const embedding = this.cleanChunk(raw, embedOpts);
      const generation = this.cleanChunk(raw, genOpts);
      pairs.push({ embedding, generation });
    }

    return { pairs, metadata: structResult.metadata };
  }

  // ── Public entry point ───────────────────────────────────────────────

  private runPipeline(content: string, opts: Partial<ChunkOptions>) {
    const options: ChunkOptions = { ...this.options, ...opts };

    // Strip YAML frontmatter, keep metadata side-channel.
    const metadata = extractYamlFrontmatter(content);
    if (metadata) {
      content = content.replace(/^---\n[\s\S]*?\n---\n?/, "");
    }

    let chunks: string[];
    if (options.chunkByHeadings) {
      chunks = this.chunkByHeadingSections(content, options);
    } else {
      chunks = this.processSection(content, options);
    }
    return { chunks, metadata };
  }

  // ── Heading-based splitting ──────────────────────────────────────────

  private chunkByHeadingSections(
    content: string,
    options: ChunkOptions,
  ): string[] {
    const sections = splitByHeadings(content, options.headingLevel);
    const allChunks: string[] = [];

    for (const section of sections) {
      const sectionHeading = this.extractSectionHeading(section, options);
      const sectionChunks = this.processSection(
        section,
        options,
        options.includeHeadingInChunks ? sectionHeading : undefined,
      );
      allChunks.push(...sectionChunks);
    }
    return allChunks;
  }

  private extractSectionHeading(
    section: string,
    options: ChunkOptions,
  ): string | undefined {
    const firstLine = section.trimStart().split("\n")[0]?.trim() ?? "";
    const headingRegex = new RegExp(`^#{${options.headingLevel ?? 1},6}\\s`);
    return headingRegex.test(firstLine) ? firstLine : undefined;
  }

  // ── Section processing pipeline ──────────────────────────────────────
  //  text → extract tables → paragraphs → glue sub-headings
  //       → raw chunks → restore tables → linearize
  //       → clean → filter oversized → prepend heading

  private processSection(
    text: string,
    options: ChunkOptions,
    sectionHeading?: string,
  ): string[] {
    // 1. Extract tables and replace with placeholders.
    const { bodyText, tables } = this.extractAndReplaceTables(text, options);

    // 2. Split into paragraph blocks.
    let paragraphs = this.splitIntoParagraphs(bodyText);

    // 3. Optionally glue sub-heading to the following paragraph.
    if (options.keepSubheadingWithContent) {
      paragraphs = this.glueSubheadingsToContent(paragraphs);
    }

    // 4. Assemble raw chunks respecting maxTokens and overlap.
    let rawChunks = this.assembleRawChunks(paragraphs, options);

    // 5. Restore table content into chunks.
    let mergedChunks = this.restoreTables(rawChunks, tables, options);

    // 5.5 Split chunks by sub-headings if requested.
    if (options.separateSubHeadings) {
      mergedChunks = this.splitChunksBySubHeadings(
        mergedChunks,
        options.headingLevel,
      );
    }

    // 6. Optionally linearize tables.
    mergedChunks = this.linearizeTableChunks(mergedChunks, options);

    // 7. Clean each chunk (formatting, links, case, etc.).
    let processed = mergedChunks
      .map((chunk) => this.cleanChunk(chunk, options))
      .filter((c) => c.length > 0);

    // 8. Split any chunk that still exceeds maxTokens.
    if (options.maxTokens > 0) {
      processed = processed.flatMap((chunk) =>
        countTokens(chunk) > options.maxTokens
          ? splitOversizedParagraph(chunk, options.maxTokens)
          : [chunk],
      );
    }

    // 9. Prepend section heading if requested.
    if (sectionHeading) {
      processed = this.prependHeadingToChunks(
        processed,
        sectionHeading,
        options,
      );
    }

    return processed;
  }

  // ── Step 1: Table extraction ─────────────────────────────────────────

  private extractAndReplaceTables(
    text: string,
    options: ChunkOptions,
  ): { bodyText: string; tables: TableInfo[] } {
    if (!options.preserveTables) {
      return { bodyText: text, tables: [] };
    }
    const extracted = extractTable(text);
    return { bodyText: extracted.replacedText, tables: extracted.tables };
  }

  // ── Step 2: Paragraph splitting ──────────────────────────────────────

  private splitIntoParagraphs(text: string): string[] {
    return text.split(/\n\s*\n/).filter((p) => p.trim().length > 0);
  }

  // ── Step 3: Glue sub-headings to following content ───────────────────

  private glueSubheadingsToContent(paragraphs: string[]): string[] {
    const result: string[] = [];
    for (let i = 0; i < paragraphs.length; i++) {
      const para = paragraphs[i];
      if (i < paragraphs.length - 1 && /^#{1,6}\s/.test(para.trimStart())) {
        result.push(para + "\n\n" + paragraphs[i + 1]);
        i++; // skip the next paragraph since it's glued
      } else {
        result.push(para);
      }
    }
    return result;
  }

  // ── Step 4: Token-aware raw chunk assembly ───────────────────────────

  private assembleRawChunks(
    paragraphs: string[],
    options: ChunkOptions,
  ): string[] {
    const rawChunks: string[] = [];
    let currentBatch: string[] = [];
    let currentTokens = 0;

    for (const para of paragraphs) {
      const pt = countTokens(para);

      // Paragraph alone exceeds limit — emit as its own chunk.
      if (pt > options.maxTokens) {
        if (currentBatch.length > 0) {
          rawChunks.push(currentBatch.join("\n\n"));
          currentBatch = [];
          currentTokens = 0;
        }
        rawChunks.push(para);
        continue;
      }

      // Would overflow — finalize current batch, start new one with overlap.
      if (currentTokens + pt > options.maxTokens) {
        rawChunks.push(currentBatch.join("\n\n"));
        currentBatch = this.computeOverlap(currentBatch, options);
        currentTokens = currentBatch.reduce(
          (sum, p) => sum + countTokens(p),
          0,
        );
      }

      currentBatch.push(para);
      currentTokens += pt;
    }

    if (currentBatch.length > 0) {
      rawChunks.push(currentBatch.join("\n\n"));
    }

    return rawChunks;
  }

  private computeOverlap(batch: string[], options: ChunkOptions): string[] {
    if (options.overlapTokens <= 0 || batch.length === 0) return [];

    const overlap: string[] = [];
    let overlapTokens = 0;
    for (let i = batch.length - 1; i >= 0; i--) {
      const t = countTokens(batch[i]);
      if (overlapTokens + t <= options.overlapTokens) {
        overlap.unshift(batch[i]);
        overlapTokens += t;
      } else {
        break;
      }
    }
    return overlap;
  }

  // ── Step 5: Table restoration ────────────────────────────────────────

  /**
   * Replace table placeholders back into chunks.
   * In "separate" mode each table becomes its own chunk, preserving
   * document order (pre-text → table → post-text).
   * Otherwise tables are inlined into the chunk that contained them.
   */
  private restoreTables(
    rawChunks: string[],
    tables: TableInfo[],
    options: ChunkOptions,
  ): string[] {
    const used = new Set<string>();
    const result: string[] = [];

    for (const chunk of rawChunks) {
      const chunkTables = tables.filter((t) => chunk.includes(t.placeholder));
      chunkTables.forEach((t) => used.add(t.placeholder));

      if (options.separateTablesAsChunks && chunkTables.length > 0) {
        // Split chunk around each table placeholder, preserving order.
        result.push(...this.splitChunkAroundTables(chunk, chunkTables));
      } else {
        // Inline all tables back into the chunk text.
        const restored = chunkTables.reduce(
          (text, t) => text.replace(t.placeholder, t.content),
          chunk,
        );
        result.push(restored);
      }
    }

    // Any orphan tables never referenced by a chunk? Emit them too.
    for (const t of tables) {
      if (!used.has(t.placeholder)) result.push(t.content);
    }

    // Drop chunks that are nothing but heading markers.
    return result.filter((chunk) => {
      const lines = chunk.split("\n").filter((l) => l.trim().length > 0);
      return !lines.every((line) => /^\s*#{1,6}\s/.test(line));
    });
  }

  /**
   * Split a chunk around its table placeholders, preserving document order:
   * text-before → table → text-between → table → text-after.
   */
  private splitChunkAroundTables(
    chunk: string,
    chunkTables: TableInfo[],
  ): string[] {
    const parts: string[] = [];
    let remaining = chunk;

    for (const t of chunkTables) {
      const idx = remaining.indexOf(t.placeholder);
      if (idx === -1) continue;

      const before = remaining.slice(0, idx).trim();
      remaining = remaining.slice(idx + t.placeholder.length);

      if (before.length > 0) parts.push(before);
      parts.push(t.content);
    }

    // Any text after the last placeholder.
    const after = remaining.trim();
    if (after.length > 0) parts.push(after);

    return parts;
  }

  /**
   * Split chunks by sub‑headings (### and below) so each sub‑heading
   * starts its own chunk. Non‑heading text between sub‑headings stays
   * with the sub‑heading above it.
   */
  private splitChunksBySubHeadings(
    chunks: string[],
    headingLevel: number,
  ): string[] {
    // Only split at levels deeper than the primary heading level.
    const subLevel = headingLevel + 1;
    if (subLevel > 6) return chunks;
    const subRegex = new RegExp(`^#{${subLevel},6}\\s`);

    return chunks.flatMap((chunk) => {
      // Pure table chunks (contain pipes but no headings) pass through.
      if (!subRegex.test(chunk) && !/^#{1,6}\s/m.test(chunk)) return [chunk];

      const lines = chunk.split("\n");
      const sections: string[] = [];
      let current: string[] = [];

      for (const line of lines) {
        if (subRegex.test(line)) {
          if (current.length > 0) sections.push(current.join("\n"));
          current = [line];
        } else {
          current.push(line);
        }
      }
      if (current.length > 0) sections.push(current.join("\n"));

      return sections.length > 0 ? sections : [chunk];
    });
  }

  // ── Step 6: Table linearization ──────────────────────────────────────

  private linearizeTableChunks(
    chunks: string[],
    options: ChunkOptions,
  ): string[] {
    if (!options.linearizeTables) return chunks;

    return chunks.flatMap((chunk) => {
      const { nonTableLines, tableBlock } = this.partitionTableLines(chunk);
      if (tableBlock.length === 0) return [chunk];

      // Linearize: each data row → descriptive sentence.
      const tableText = tableBlock.join("\n");
      let rows = linearizeTable(tableText);

      // Light cleaning pass on each linearized row.
      rows = rows.map((row) =>
        this.cleanChunk(row, {
          removeRules: options.removeRules,
          removeNumbering: options.removeNumbering,
          removeListMarkers: options.removeListMarkers,
          removeLinks: options.removeLinks,
          formatLatex: options.formatLatex,
          removeFormatting: options.removeFormatting,
        } as ChunkOptions),
      );

      // Either emit each row as its own chunk, or batch respecting maxTokens.
      const subChunks = options.eachTableRowAsSeparateChunk
        ? rows
        : this.assembleRawChunks(rows, options);

      // Prepend any non-table prefix lines to the first sub-chunk.
      if (nonTableLines.length > 0 && subChunks.length > 0) {
        subChunks[0] = nonTableLines.join("\n") + "\n" + subChunks[0];
      } else if (nonTableLines.length > 0) {
        subChunks.push(nonTableLines.join("\n"));
      }

      return subChunks.length > 0 ? subChunks : [chunk];
    });
  }

  /**
   * Split a chunk's lines into non-table prefix lines and a table block.
   */
  private partitionTableLines(chunk: string): {
    nonTableLines: string[];
    tableBlock: string[];
  } {
    if (!/\|/.test(chunk))
      return { nonTableLines: chunk.split("\n"), tableBlock: [] };

    const lines = chunk.split("\n");
    const nonTableLines: string[] = [];
    const tableBlock: string[] = [];
    let insideTable = false;

    for (const line of lines) {
      if (/^\|/.test(line.trim())) {
        tableBlock.push(line);
        insideTable = true;
      } else if (insideTable) {
        insideTable = false;
        nonTableLines.push(line);
      } else {
        nonTableLines.push(line);
      }
    }
    return { nonTableLines, tableBlock };
  }

  // ── Step 7: Per-chunk cleaning ───────────────────────────────────────

  private cleanChunk(chunk: string, options: ChunkOptions): string {
    let c = chunk;
    if (options.removeRules) c = removeHorizontalRules(c);
    if (options.removeNumbering) c = removeListNumbering(c);
    if (options.removeListMarkers) c = removeListMarkers(c);
    if (options.removeLinks) c = removeMarkdownLinks(c);
    if (options.formatLatex) c = formatLatex(c);
    if (options.removeFormatting) c = removeMarkdownFormatting(c);
    if (options.stripHeadingMarkers) c = stripHeadingMarkers(c);
    if (options.lowerCase) c = c.toLowerCase();
    if (options.compactLines) c = removeEmptyLines(c);
    return c.trim();
  }

  // ── Step 9: Prepend heading to each chunk ────────────────────────────

  private prependHeadingToChunks(
    chunks: string[],
    sectionHeading: string,
    options: ChunkOptions,
  ): string[] {
    const cleanHeading = this.cleanChunk(sectionHeading, options).trim();

    return (
      chunks
        // Don't duplicate if it's already the first line.
        .filter((chunk) => chunk !== cleanHeading)
        .map((chunk) => {
          const firstLine = chunk.trimStart().split("\n")[0]?.trim() ?? "";
          if (firstLine === cleanHeading) return chunk;
          return cleanHeading + "\n" + chunk;
        })
    );
  }
}

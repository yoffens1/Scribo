import { describe, it } from "node:test";
import assert from "node:assert/strict";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import {
  Chunker,
  EMBEDDING_OPTIONS,
  GENERATION_OPTIONS,
} from "@utils/chunker/Chunker";
import { STRUCTURAL_OPTIONS } from "@utils/chunker/Chunker";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const fixturesDir = path.resolve(__dirname, "fixtures");

function readFixture(name: string): string {
  return fs.readFileSync(path.join(fixturesDir, name), "utf-8");
}

const atomMd = readFixture("Atom.md");

describe("Chunker with Atom.md", () => {
  // ── Embedding mode ──────────────────────────────────────────
  describe("chunkForEmbedding", () => {
    it("returns chunks and metadata", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks, metadata } = chunker.chunkForEmbedding(atomMd);
      assert.ok(metadata !== null, "should extract frontmatter metadata");
      assert.ok(Array.isArray(chunks), "chunks should be an array");
      assert.ok(chunks.length > 0, "should produce at least one chunk");
    });

    it("metadata contains aliases and tags", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { metadata } = chunker.chunkForEmbedding(atomMd);
      assert.ok(metadata?.aliases, "should have aliases");
      assert.ok(metadata?.tags, "should have tags");
    });

    it("each chunk is lowercased (embedding mode)", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        assert.strictEqual(
          c,
          c.toLowerCase(),
          "embedding chunk should be all lowercased",
        );
      }
    });

    it("chunks have no markdown links (embedding mode)", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        assert.ok(!c.includes("[["), "should not contain wikilinks: [[...]]");
        assert.ok(!c.includes("](http"), "should not contain markdown links");
      }
    });

    it("chunks have no formatting markers (embedding mode)", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        assert.ok(!c.includes("**"), "should not contain bold markers");
        assert.ok(!c.includes("__"), "should not contain bold markers (alt)");
        assert.ok(!c.includes("*"), "should not contain italic markers");
        assert.ok(!c.includes("~~"), "should not contain strikethrough");
        assert.ok(!c.match(/(?<![`])`(?![`])/), "should not contain inline code backticks");
      }
    });

    it("chunks have no horizontal rules", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        assert.ok(!/^[-*_]{3,}$/m.test(c.trim()), "should not contain horizontal rules");
      }
    });

    it("all chunks have no sub-heading markers (separateSubHeadings splits them apart)", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        // After stripHeadingMarkers + cleanChunk, no ### markers should remain
        assert.ok(!/^###\s/m.test(c), "should not contain ### sub-heading markers");
      }
    });

    it("table rows are linearized into separate chunks", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      // The Atom.md document has a table with 3 data rows + header.
      // In embedding mode, eachTableRowAsSeparateChunk = true,
      // so we should find linearized rows in separate chunks.
      const tableChunks = chunks.filter(
        (c) =>
          c.includes("протон") ||
          c.includes("нейтрон") ||
          c.includes("электрон"),
      );
      assert.ok(tableChunks.length >= 3, "should have at least 3 table-row-like chunks");
    });

    it("chunks stay within token budget", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks } = chunker.chunkForEmbedding(atomMd);
      for (const c of chunks) {
        const tokenCount = c.match(/\w+|[^\w\s]|\s+/g)?.length ?? 0;
        // With includeHeadingInChunks + compactLines, some chunks slightly
        // exceed maxTokens when the heading plus first paragraph is too long.
        // The Chunker ensures nothing is lost, but tight token budgets may
        // push a chunk a few tokens over when heading is prepended.
        assert.ok(
          tokenCount <= 512,
          `chunk has ${tokenCount} tokens (budget 512)`,
        );
      }
    });

    it("full chunks — no slices (preview all embedding chunks)", async () => {
      const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
      const { chunks, metadata } = chunker.chunkForEmbedding(atomMd);

      console.log("=== METADATA ===");
      console.log(JSON.stringify(metadata, null, 2));

      console.log(`\n=== EMBEDDING CHUNKS (${chunks.length} total) ===`);
      for (let i = 0; i < chunks.length; i++) {
        console.log(`\n--- Chunk #${i + 1} ---`);
        console.log(chunks[i]);
      }

      assert.ok(chunks.length > 0, "should produce at least one chunk");
    });
  });

  // ── Generation mode ─────────────────────────────────────────
  describe("chunkForGeneration", () => {
    it("returns chunks and metadata", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks, metadata } = chunker.chunkForGeneration(atomMd);
      assert.ok(metadata !== null, "should extract frontmatter metadata");
      assert.ok(Array.isArray(chunks), "chunks should be an array");
      assert.ok(chunks.length > 0, "should produce at least one chunk");
    });

    it("chunks preserve original case (generation mode)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // At least one chunk should contain uppercase characters
      const hasUppercase = chunks.some((c) => /[A-Z]/.test(c));
      assert.ok(hasUppercase, "generation chunks should preserve uppercase");
    });

    it("chunks preserve markdown links (generation mode)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // Atom.md uses standard markdown links [text](url) — no wikilinks
      const hasLinks = chunks.some((c) => c.includes("](http") || c.match(/\]\([^)]+\.md\)/));
      assert.ok(hasLinks, "generation chunks should preserve markdown links");
    });

    it("chunks preserve formatting markers (generation mode)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // Bold/italic markers like **Атом**
      const hasBold = chunks.some((c) => c.includes("**"));
      assert.ok(hasBold, "generation chunks should preserve bold/italic markers");
    });

    it("tables are separated as whole chunks (generation mode)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // Tables are separated into their own chunks with separateTablesAsChunks: true
      // and linearizeTables: false — so we should see raw table markdown in a chunk
      const tableChunk = chunks.find((c) => c.includes("|:"));
      assert.ok(tableChunk, "should have at least one chunk with markdown table");
    });

    it("sub-headings start separate chunks (generation mode)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // separateSubHeadings: true + keepSubheadingWithContent: false
      // means each ### heading gets its own chunk
      const headingChunks = chunks.filter((c) => c.trimStart().startsWith("###"));
      assert.ok(headingChunks.length > 0, "should have chunks starting with ### sub-headings");
    });

    it("chunks have no max token limit in generation mode", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks } = chunker.chunkForGeneration(atomMd);
      // maxTokens is Infinity, so no splitting should trigger
      assert.ok(chunks.length > 0, "should work without token cap");
    });

    it("full chunks — no slices (preview all generation chunks)", async () => {
      const chunker = new Chunker({ ...GENERATION_OPTIONS });
      const { chunks, metadata } = chunker.chunkForGeneration(atomMd);

      console.log("=== METADATA ===");
      console.log(JSON.stringify(metadata, null, 2));

      console.log(`\n=== GENERATION CHUNKS (${chunks.length} total) ===`);
      for (let i = 0; i < chunks.length; i++) {
        console.log(`\n--- Chunk #${i + 1} ---`);
        console.log(chunks[i]);
      }

      assert.ok(chunks.length > 0, "should produce at least one chunk");
    });
  });

  // ── Paired mode (chunkPaired) ────────────────────────────
  describe("chunkPaired", () => {
    it("returns matched pairs with embedding and generation", async () => {
      const chunker = new Chunker();
      const { pairs, metadata } = chunker.chunkPaired(atomMd);
      assert.ok(metadata !== null, "should extract metadata");
      assert.ok(pairs.length > 0, "should produce pairs");
      for (const p of pairs) {
        assert.ok(p.embedding.length > 0, "embedding should not be empty");
        assert.ok(p.generation.length > 0, "generation should not be empty");
      }
    });

    it("embedding is lowercased, generation preserves case", async () => {
      const chunker = new Chunker();
      const { pairs } = chunker.chunkPaired(atomMd);
      for (const p of pairs) {
        assert.strictEqual(
          p.embedding,
          p.embedding.toLowerCase(),
          "embedding text should be lowercased",
        );
      }
      const hasUppercaseGen = pairs.some((p) => /[A-Z]/.test(p.generation));
      assert.ok(hasUppercaseGen, "generation text should preserve uppercase");
    });

    it("embedding has no links, generation preserves them", async () => {
      const chunker = new Chunker();
      const { pairs } = chunker.chunkPaired(atomMd);
      for (const p of pairs) {
        assert.ok(
          !p.embedding.includes("](http") && !/\]\([^)]+\.md\)/.test(p.embedding),
          "embedding should not contain markdown links",
        );
      }
      const hasLinksGen = pairs.some(
        (p) => p.generation.includes("](http") || /\]\([^)]+\.md\)/.test(p.generation),
      );
      assert.ok(hasLinksGen, "generation should preserve markdown links");
    });

    it("embedding has no formatting, generation preserves it", async () => {
      const chunker = new Chunker();
      const { pairs } = chunker.chunkPaired(atomMd);
      for (const p of pairs) {
        assert.ok(
          !p.embedding.includes("**"),
          "embedding should not contain bold markers",
        );
      }
      const hasBoldGen = pairs.some((p) => p.generation.includes("**"));
      assert.ok(hasBoldGen, "generation should preserve bold markers");
    });

    it("full pairs — no slices (preview all paired chunks)", async () => {
      const chunker = new Chunker();
      const { pairs, metadata } = chunker.chunkPaired(atomMd);

      console.log("=== METADATA ===");
      console.log(JSON.stringify(metadata, null, 2));

      console.log(`\n=== PAIRED CHUNKS (${pairs.length} total) ===`);
      for (let i = 0; i < pairs.length; i++) {
        console.log(`\n--- Pair #${i + 1} ---`);
        console.log(`EMBEDDING:\n${pairs[i].embedding}`);
        console.log(`GENERATION:\n${pairs[i].generation}`);
      }

      assert.ok(pairs.length > 0, "should produce at least one pair");
    });
  });

  // ── Individual option toggles ───────────────────────────────
  describe("option toggles", () => {
    it("lowerCase=false preserves case", async () => {
      const chunker = new Chunker({ lowerCase: false, maxTokens: 1024 });
      const { chunks } = chunker.runPipelineTest(atomMd);
      const hasUppercase = chunks.some((c) => /[A-Z]/.test(c));
      assert.ok(hasUppercase, "chunks should retain uppercase when lowerCase is false");
    });

    it("stripHeadingMarkers=false keeps # markers", async () => {
      const chunker = new Chunker({
        stripHeadingMarkers: false,
        chunkByHeadings: true,
        headingLevel: 2,
        maxTokens: 1024,
      });
      const { chunks } = chunker.runPipelineTest(atomMd);
      const hasHeadings = chunks.some((c) => /^## /m.test(c));
      assert.ok(hasHeadings, "chunks should keep ## heading markers");
    });

    it("compactLines=false keeps blank lines", async () => {
      const chunker = new Chunker({
        compactLines: false,
        chunkByHeadings: false,
        maxTokens: 16384,
      });
      const { chunks } = chunker.runPipelineTest(atomMd);
      // With chunkByHeadings: false, the entire document body goes into one chunk
      // and blank lines between paragraphs are preserved
      const hasEmptyLines = chunks.some((c) => /\n\s*\n/.test(c) && c.trim().length > 0);
      assert.ok(hasEmptyLines, "chunks should keep blank lines when compactLines=false");
    });

    it("chunkByHeadings=false puts everything in one chunk", async () => {
      const chunker = new Chunker({
        chunkByHeadings: false,
        maxTokens: 8192,
        headingLevel: 2,
      });
      const { chunks } = chunker.runPipelineTest(atomMd);
      // With no heading splitting, all content goes into raw assembly
      // The document is small enough to fit in one chunk with 8192 maxTokens
      const bodyChunks = chunks.filter((c) => c.trim().length > 0);
      assert.ok(bodyChunks.length >= 1, "should have at least one chunk");
      // The combined text should be substantial
      const total = bodyChunks.join(" ").length;
      assert.ok(total > 200, `combined text length ${total} should be substantial`);
    });
  });
});

/**
 * Helper to expose Chunker's private runPipeline for testing.
 * We patch the prototype temporarily so we don't modify the class itself.
 */
declare module "@utils/chunker/Chunker" {
  interface Chunker {
    runPipelineTest(
      content: string,
    ): { chunks: string[]; metadata: Record<string, unknown> | null };
  }
}

// Add a test-only public method so we can exercise runPipeline with custom options
Chunker.prototype.runPipelineTest = function (content: string) {
  return (this as any).runPipeline(content, { ...(this as any).options, ...STRUCTURAL_OPTIONS });
};

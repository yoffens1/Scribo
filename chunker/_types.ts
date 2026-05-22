export interface ChunkOptions {
  /** Maximum number of tokens allowed in a single chunk. */
  maxTokens: number;
  /** Number of tokens that overlap between consecutive chunks. */
  overlapTokens: number;
  /** If true, tables are extracted before chunking and re-inserted without breaking them. */
  preserveTables: boolean;
  /** Convert all text to lower case (useful for embeddings). */
  lowerCase: boolean;
  /** Remove Markdown links: [[wikilinks]] and [text](url). */
  removeLinks: boolean;
  /** Remove inline formatting: **bold**, *italic*, ==highlight==, ~~strikethrough~~, `code`. */
  removeFormatting: boolean;
  /** Format LaTeX math expressions: $inline$ and $$block$$. to utf-8 */
  formatLatex: boolean;
  /** Turn Markdown tables into descriptive sentences (good for embeddings). */
  linearizeTables: boolean;
  /** Split the document by Markdown headings (#, ##, ###, etc.). */
  chunkByHeadings: boolean;
  /** Prefix each chunk with its parent heading so context is preserved. */
  includeHeadingInChunks: boolean;
  /** Remove horizontal rules (---, ***, ___) from chunks. */
  removeRules: boolean;
  /** Heading level used for the initial section split (1 for #, 2 for ##, etc.). */
  headingLevel: number;
  /** Force a new chunk at every sub‑heading (###, ####, …). */
  separateSubHeadings: boolean;
  /** Collapse multiple blank lines into a single line. */
  compactLines?: boolean;
  /** Strip leading numbering like "1.", "2.1.", "IV." from lines. */
  removeNumbering?: boolean;
  /** Remove '#' markers from heading text (keeps only the text). */
  stripHeadingMarkers?: boolean;
  /** Glue a sub‑heading to the next paragraph so they stay in the same chunk. */
  keepSubheadingWithContent?: boolean;
  /** Remove Markdown list markers like '-', '*', '+' at the start of lines. */
  removeListMarkers?: boolean;
  /** If true, each table becomes a separate chunk (for generation mode). */
  separateTablesAsChunks?: boolean;
  eachTableRowAsSeparateChunk: boolean;
}

export type TableInfo = {
  placeholder: string;
  content: string;
  tokens: number;
};

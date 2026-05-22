// src/test/cli/refinery.test.ts
//
// CLI for testing the refinery pipeline outside Obsidian.
//
// Usage:
//   npm run refinery plan <file.md>      — dry-run: show plan without writing
//   npm run refinery refine <file.md>    — run pipeline and write to output
//   npm run refinery refine <file.md> --dry-run  — plan only
//   npm run refinery refine <file.md> --apply     — actually write files
//
// Options:
//   --input <dir>    Input folder (default: src/test/refinery/fixtures)
//   --output <dir>   Output folder (default: src/test/test-db/refinery-output)
//   --llm <model>    LLM model for Ollama (default: llama3.2:3b)
//   --dry-run        Show plan without writing (default behavior)
//   --apply          Actually write files to output folder

import * as path from "path";
import * as fs from "fs/promises";
import { RefineryService } from "@refinery/RefineryService";
import type { RefineryResult } from "@refinery/types/refinery-result";
import type { IFileAccess } from "@utils/_types";
import { LLMService } from "@ai/llm/LLMService";
import { Logger, ConsoleSink } from "@logging/index";

// ── Filesystem adapter ──

const createFsAdapter = (root: string): IFileAccess => ({
  async readBinary(normalizedPath: string) {
    const buf = await fs.readFile(path.join(root, normalizedPath));
    return buf.buffer.slice(
      buf.byteOffset,
      buf.byteOffset + buf.byteLength,
    ) as ArrayBuffer;
  },
  async readText(normalizedPath: string) {
    return fs.readFile(path.join(root, normalizedPath), "utf-8");
  },
  async writeText(p: string, content: string) {
    const full = path.join(root, p);
    await fs.mkdir(path.dirname(full), { recursive: true });
    await fs.writeFile(full, content, "utf-8");
  },
  async exists(normalizedPath: string) {
    try {
      await fs.access(path.join(root, normalizedPath));
      return true;
    } catch {
      return false;
    }
  },
  async list(p: string) {
    try {
      const entries = await fs.readdir(path.join(root, p), {
        withFileTypes: true,
      });
      return entries
        .filter((e) => !e.name.startsWith("."))
        .map((e) => ({ name: e.name, isDir: e.isDirectory() }));
    } catch {
      return [];
    }
  },
  async rename(from: string, to: string) {
    await fs.rename(path.join(root, from), path.join(root, to));
  },
  async delete(p: string) {
    try {
      await fs.unlink(path.join(root, p));
    } catch {
      /* ignore */
    }
  },
});

// ── Mock retrieval (no embedding index for CLI) ──

const noopRetrieval = {
  query: async () => [],
  setEmbedder: () => {},
  markDirty: () => {},
  dispose: () => {},
} as any;

// ── Output formatting ──

const printDivider = (title: string) => {
  console.log(`\n${"═".repeat(60)}`);
  console.log(`  ${title}`);
  console.log(`${"═".repeat(60)}`);
};

const printChunks = (result: RefineryResult) => {
  printDivider("CHUNKS");
  console.log(`Source: ${result.sourcePath}`);
  console.log(`Total chunks: ${result.chunks.length}`);
  result.chunks.forEach((c, i) => {
    console.log(`\n  [${i}] hash=${c.hash.slice(0, 8)}...`);
    console.log(
      `      ${c.text.slice(0, 100)}${c.text.length > 100 ? "..." : ""}`,
    );
  });
};

const printDedup = (result: RefineryResult) => {
  printDivider("DEDUPLICATION");
  const remaining = result.dedup.remaining;
  const decisions = result.dedup.decisions;
  console.log(`Kept: ${remaining.length} / ${result.chunks.length} chunks`);
  const rejected = result.chunks.length - remaining.length;
  if (rejected > 0) {
    console.log(`Rejected/merged: ${rejected}`);
    decisions
      .filter((d) => d.action !== "keep")
      .forEach((d) => {
        console.log(`  [${d.action}] ${d.reason.slice(0, 80)}`);
      });
  }
  remaining.forEach((c, i) => {
    console.log(`  [${i}] ${c.text.slice(0, 80)}...`);
  });
};

const printTaxonomy = (result: RefineryResult) => {
  printDivider("TAXONOMY");
  console.log(`Rationale: ${result.taxonomy.rationale}`);
  const printTree = (nodes: any[], indent = 0) => {
    for (const n of nodes) {
      console.log(
        `${"  ".repeat(indent)}📁 ${n.name}/  (${n.assignedChunks.length} chunks)`,
      );
      if (n.description)
        console.log(`${"  ".repeat(indent + 1)}${n.description}`);
      printTree(n.children, indent + 1);
    }
  };
  printTree(result.taxonomy.roots);
};

const printPlacement = (result: RefineryResult) => {
  printDivider("PLACEMENT");
  console.log(`Rationale: ${result.placement.rationale}`);
  if (result.placement.foldersToCreate.length > 0) {
    console.log(`\nFolders to create:`);
    result.placement.foldersToCreate.forEach((f) => console.log(`  📁 ${f}`));
  }
  console.log(`\nChunk decisions:`);
  result.placement.decisions.forEach((d, i) => {
    const icon =
      d.action === "create"
        ? "📄"
        : d.action === "merge"
          ? "🔗"
          : d.action === "nest"
            ? "📁"
            : "✏️";
    console.log(`  ${icon} [${d.action}] ${d.outputPath}`);
    console.log(`      ${d.reason}`);
  });
};

const printOperations = (result: RefineryResult) => {
  printDivider("OPERATIONS");
  if (result.dryRun) {
    console.log("⚠️  DRY RUN — no files were written");
  }
  console.log(`Total operations: ${result.operations.length}`);
  result.operations.forEach((op, i) => {
    const icon =
      op.type === "create_file"
        ? "📄"
        : op.type === "create_folder"
          ? "📁"
          : op.type === "merge_chunk"
            ? "🔗"
            : "✏️";
    console.log(
      `  ${i + 1}. ${icon} ${op.type}: ${(op as any).path ?? (op as any).targetFile ?? (op as any).from}`,
    );
  });
};

// ── Main ──

async function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  if (!command || (command !== "plan" && command !== "refine")) {
    console.log(`
Refinery CLI — test the refinery pipeline outside Obsidian.

Usage:
  npm run refinery plan <file.md>        Dry-run: show plan without writing
  npm run refinery refine <file.md>      Run pipeline (dry-run by default)
  npm run refinery refine <file.md> --apply   Actually write files

Options:
  --input <dir>    Input folder (default: src/test/refinery/fixtures)
  --output <dir>   Output folder (default: src/test/test-db/refinery-output)
  --llm <model>    Ollama model (default: llama3.2:3b)
  --dry-run        Show plan without writing (default)
  --apply          Actually write files to output
`);
    return;
  }

  const file = args[1];
  if (!file) {
    console.error(
      "❌ File path required. Usage: npm run refinery plan <file.md>",
    );
    return;
  }

  // Parse options
  const inputRoot = args.includes("--input")
    ? args[args.indexOf("--input") + 1]
    : "src/test/refinery/fixtures";
  const outputRoot = args.includes("--output")
    ? args[args.indexOf("--output") + 1]
    : "src/test/test-db/refinery-output";
  const llmModel = args.includes("--llm")
    ? args[args.indexOf("--llm") + 1]
    : "llama3.2:3b";
  const dryRun = !args.includes("--apply"); // dry-run by default

  console.log(`📂 Input:  ${inputRoot}`);
  console.log(`📂 Output: ${outputRoot}`);
  console.log(`🤖 LLM:    ${llmModel}`);
  console.log(
    `🔧 Mode:   ${dryRun ? "DRY RUN (plan only)" : "APPLY (will write files)"}`,
  );
  console.log();

  // Check input file exists
  const inputPath = path.join(inputRoot, file);
  try {
    await fs.access(inputPath);
  } catch {
    console.error(`❌ File not found: ${inputPath}`);
    return;
  }

  // Ensure output directory exists
  await fs.mkdir(outputRoot, { recursive: true });

  // Setup services
  const fileAccess = createFsAdapter(inputRoot);
  const logger = new Logger("refinery-cli", {
    enabled: true,
    minLevel: "info",
    sinks: [new ConsoleSink()],
  });

  const llm = new LLMService({
    backend: "ollama",
    model: llmModel,
    temperature: 0.3,
  } as any);
  console.log(`✅ LLM connected: ${llmModel}`);

  const svc = new RefineryService({
    fileAccess,
    retrieval: noopRetrieval,
    llm,
    logger,
    outputRoot,
    inboxRoot: "", // adapter already rooted at input
    dryRun,
  });

  try {
    if (command === "plan") {
      console.log(`\n🔍 Planning for: ${file}`);
      const plan = await svc.plan(file);

      // Convert plan to RefineryResult-like shape for printing
      const mockResult: RefineryResult = {
        sourcePath: file,
        chunks: plan.chunks,
        dedup: plan.dedup,
        taxonomy: plan.taxonomy,
        placement: plan.placement,
        operations: [],
        dryRun: true,
      };
      printChunks(mockResult);
      printDedup(mockResult);
      printTaxonomy(mockResult);
      printPlacement(mockResult);
      console.log(`\n✅ Plan complete.`);
    } else {
      console.log(`\n🔄 Refining: ${file}`);
      const t0 = performance.now();
      const result = await svc.refine(file);
      const elapsed = (performance.now() - t0).toFixed(0);

      printChunks(result);
      printDedup(result);
      printTaxonomy(result);
      printPlacement(result);
      printOperations(result);
      console.log(`\n✅ Refined in ${elapsed}ms.`);
    }
  } catch (err) {
    console.error(`\n❌ Error:`, err);
  }
}

main().catch(console.error);

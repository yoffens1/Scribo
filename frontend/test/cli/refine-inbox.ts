// src/test/cli/refine-inbox.ts
//
// Single-file refinery from the inbox folder.
// Just pass the filename — no full paths needed.
//
// Usage:
//   npm run inbox Atom.md                    dry-run (show plan)
//   npm run inbox Atom.md --apply             write to output
//   npm run inbox Atom.md --apply --llm llama3.2:3b

import * as path from "path";
import * as fs from "fs/promises";
import { RefineryService } from "@refinery/RefineryService";
import type { RefineryResult } from "@refinery/types/refinery-result";
import type { IFileAccess } from "@utils/_types";
import { LLMService } from "@ai/llm/LLMService";
import { Logger, ConsoleSink } from "@logging/index";
import { PATH_TO_VAULT } from "@settings";

const INBOX = PATH_TO_VAULT.inputVault;
const DB_DIR = PATH_TO_VAULT.outputPathForDb;

// ── Adapter: READ from INBOX, WRITE using absolute paths ──

const adapter = (readRoot: string): IFileAccess => ({
  async readBinary(p: string) {
    const buf = await fs.readFile(path.join(readRoot, p));
    return buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength) as ArrayBuffer;
  },
  async readText(p: string) { return fs.readFile(path.join(readRoot, p), "utf-8"); },
  async writeText(p: string, content: string) {
    const full = path.isAbsolute(p) ? p : path.join(readRoot, p);
    await fs.mkdir(path.dirname(full), { recursive: true });
    await fs.writeFile(full, content, "utf-8");
  },
  async exists(p: string) {
    try { await fs.access(path.join(readRoot, p)); return true; } catch { return false; }
  },
  async list(p: string) {
    const target = path.isAbsolute(p) ? p : path.join(readRoot, p);
    try {
      const entries = await fs.readdir(target, { withFileTypes: true });
      return entries.filter(e => !e.name.startsWith(".")).map(e => ({ name: e.name, isDir: e.isDirectory() }));
    } catch { return []; }
  },
  async rename(from: string, to: string) {
    const f = path.isAbsolute(from) ? from : path.join(readRoot, from);
    const t = path.isAbsolute(to) ? to : path.join(readRoot, to);
    await fs.rename(f, t);
  },
  async delete(p: string) {
    try { await fs.unlink(path.isAbsolute(p) ? p : path.join(readRoot, p)); } catch { /* ignore */ }
  },
});

const noopRetrieval = {
  query: async () => [], setEmbedder: () => {}, markDirty: () => {}, dispose: () => {},
} as any;

// ── Output ──

const hr = (title: string) => console.log(`\n${"─".repeat(55)}\n  ${title}\n${"─".repeat(55)}`);

const printResult = (r: RefineryResult) => {
  hr("CHUNKS");
  console.log(`${r.sourcePath}: ${r.chunks.length} chunks`);
  r.chunks.forEach((c, i) => console.log(`  [${i}] ${c.hash.slice(0, 10)}  ${c.text.slice(0, 90)}${c.text.length > 90 ? "…" : ""}`));

  hr("DEDUP");
  console.log(`Kept: ${r.dedup.remaining.length} / ${r.chunks.length}`);

  hr("TAXONOMY");
  console.log(r.taxonomy.rationale);
  const tree = (nodes: any[], d = 0) => nodes.forEach(n => {
    console.log(`${"  ".repeat(d)}📁 ${n.name}/  (${n.assignedChunks.length} chunks)`);
    tree(n.children, d + 1);
  });
  tree(r.taxonomy.roots);

  hr("PLACEMENT");
  r.placement.decisions.forEach(d =>
    console.log(`  ${d.action === "create" ? "📄" : d.action === "merge" ? "🔗" : "📁"} [${d.action}] ${d.outputPath}`));

  hr("OPERATIONS");
  console.log(`  Total: ${r.operations.length}`);
  r.operations.forEach((op, i) =>
    console.log(`  ${i + 1}. ${op.type}: ${(op as any).path ?? (op as any).targetFile ?? ""}`));
};

// ── Real filesystem writer ──

const writeToDisk = async (ops: RefineryResult["operations"]) => {
  let created = 0;
  for (const op of ops) {
    if (op.type === "create_file") {
      await fs.mkdir(path.dirname(op.path), { recursive: true });
      await fs.writeFile(op.path, op.content, "utf-8");
      console.log(`  ✅ wrote ${op.path}`);
      created++;
    } else if (op.type === "create_folder") {
      await fs.mkdir(op.path, { recursive: true });
      console.log(`  📁 created ${op.path}`);
      created++;
    } else if (op.type === "merge_chunk") {
      if (!op.targetFile) continue;
      // Ensure parent directory exists
      await fs.mkdir(path.dirname(op.targetFile), { recursive: true });
      try {
        const existing = await fs.readFile(op.targetFile, "utf-8");
        
        // Проверка: если текст уже есть в файле, пропускаем
        const normalize = (s: string) => s.replace(/[\s\r\n]+/g, " ").trim().toLowerCase();
        if (normalize(existing).includes(normalize(op.chunkText))) {
          console.log(`  🔗 skipped merge into ${op.targetFile} (content already exists)`);
          continue;
        }

        await fs.writeFile(op.targetFile, existing + "\n\n---\n\n" + op.chunkText, "utf-8");
      } catch {
        await fs.writeFile(op.targetFile, op.chunkText, "utf-8");
      }
      console.log(`  🔗 merged into ${op.targetFile}`);
      created++;
    } else if (op.type === "move_file") {
      await fs.rename(op.from, op.to);
      console.log(`  ✏️  moved ${op.from} → ${op.to}`);
      created++;
    }
  }
  console.log(`\n  ✅ ${created} operation(s) applied.`);
};

// ── Argument parsing ──

const parseArgs = (args: string[]) => {
  let fileName = "";
  let apply = false;
  let outputRoot = path.resolve(DB_DIR, "refinery-output");
  let llmModel = "llama3.2:3b";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--apply") apply = true;
    else if (args[i] === "--output" && i + 1 < args.length) outputRoot = path.resolve(args[++i]);
    else if (args[i] === "--llm" && i + 1 < args.length) llmModel = args[++i];
    else if (!args[i].startsWith("-") && !fileName) fileName = args[i];
  }
  return { fileName, apply, outputRoot, llmModel };
};

// ── Main ──

async function main() {
  const args = process.argv.slice(2);
  const { fileName, apply, outputRoot, llmModel } = parseArgs(args);

  if (!fileName) {
    console.log(`\ninbox — refine a single file from ${INBOX}\n\nUsage:\n  npm run inbox Atom.md\n  npm run inbox Atom.md --apply\n  npm run inbox Atom.md --apply --llm llama3.2:3b\n`);
    return;
  }

  const inputPath = path.join(INBOX, fileName);
  try { await fs.access(inputPath); } catch {
    console.error(`❌ Not found: ${inputPath}`); return;
  }
  await fs.mkdir(outputRoot, { recursive: true });

  console.log(`📥 ${INBOX}`);
  console.log(`📤 ${outputRoot}`);
  console.log(`🤖 ${llmModel}`);
  console.log(`🔧 ${apply ? "APPLY" : "DRY RUN"}\n`);

  const svc = new RefineryService({
    fileAccess: adapter(INBOX),
    retrieval: noopRetrieval,
    llm: new LLMService({ backend: "ollama", model: llmModel, temperature: 0.3, responseFormat: "json" } as any),
    logger: new Logger("inbox", { minLevel: "info", sinks: [new ConsoleSink()] }),
    outputRoot,
    inboxRoot: "",
    dryRun: true,
  });

  const t0 = performance.now();
  const result = await svc.refine(fileName, { dryRun: !apply });
  console.log(`\n⏱️  planned in ${(performance.now() - t0).toFixed(0)}ms\n`);
  printResult(result);

  if (apply) {
    console.log(`\n${"═".repeat(55)}\n  APPLYING to disk...\n${"═".repeat(55)}`);
    await writeToDisk(result.operations);
  }
}

main().catch(console.error);

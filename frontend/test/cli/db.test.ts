// src/test/cli/db.test.ts
import * as path from "path";
import * as fs from "fs/promises";
import { VectorDatabase } from "@database/Database";
import { Embedder } from "@ai/embedding/Embedder";
import { Chunker, EMBEDDING_OPTIONS } from "@utils/chunker/Chunker";
import { LLMService } from "@ai/llm/LLMService";
import { Translator } from "@translation/Translator";
import { PATH_TO_VAULT, TRANSLATOR_CONFIG } from "@settings";
import { mergeByFile } from "@retrieval/utils/mergeByFile";

const INPUT_PATH = PATH_TO_VAULT.inputVault;
const OUTPUT_PATH = PATH_TO_VAULT.outputPathForDb;
const MODEL_NAME = "qwen3-embedding:latest";

function createFsAdapter(vaultRoot: string) {
  return {
    async readBinary(normalizedPath: string) {
      const buf = await fs.readFile(path.join(vaultRoot, normalizedPath));
      return buf.buffer.slice(
        buf.byteOffset,
        buf.byteOffset + buf.byteLength,
      ) as ArrayBuffer;
    },
    async writeBinary(normalizedPath: string, data: ArrayBuffer) {
      const fullPath = path.join(vaultRoot, normalizedPath);
      await fs.mkdir(path.dirname(fullPath), { recursive: true });
      await fs.writeFile(fullPath, new Uint8Array(data));
    },
    async exists(normalizedPath: string) {
      try {
        await fs.access(path.join(vaultRoot, normalizedPath));
        return true;
      } catch {
        return false;
      }
    },
    async remove(normalizedPath: string) {
      await fs.unlink(path.join(vaultRoot, normalizedPath));
    },
  };
}

async function collectMdFiles(dir: string): Promise<string[]> {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    if (entry.name.startsWith(".")) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectMdFiles(full)));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      files.push(full);
    }
  }
  return files;
}

async function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  const safeModel = MODEL_NAME.replace(/[^a-zA-Z0-9-_]/g, "-");
  const dbFile = path.join(OUTPUT_PATH, `vectors-${safeModel}.db`);
  console.log(`📁 DB path: ${dbFile}\n`);

  const adapter = createFsAdapter(OUTPUT_PATH);
  const db = new VectorDatabase(adapter as any, ".", MODEL_NAME);
  await db.initialize();

  const chunker = new Chunker({ ...EMBEDDING_OPTIONS, maxTokens: 256 });
  const embedder = new Embedder({
    provider: "ollama",
    model: MODEL_NAME,
    chunker,
  });
  await embedder.initialize();

  // Pipeline deps — optional, only created when needed
  let llm: LLMService | undefined;
  let translator: Translator | undefined;

  const printChunk = (chunk: {
    filePath?: string;
    chunkIndex: number;
    chunkText?: string;
    embedding?: Float32Array;
    tokenCount?: number;
  }) => {
    if (chunk.filePath) console.log(`📄 ${chunk.filePath}`);
    console.log(`   chunk #${chunk.chunkIndex}`);
    if (chunk.chunkText) console.log(`   text: ${chunk.chunkText.slice(0, 80)}${chunk.chunkText.length > 80 ? "..." : ""}`);
    if (chunk.tokenCount !== undefined) console.log(`   tokens: ${chunk.tokenCount}`);
    if (chunk.embedding) console.log(`   embedding: [${chunk.embedding.length} floats] ${Array.from(chunk.embedding.slice(0, 3)).join(", ")}...`);
    console.log();
  };

  try {
    switch (command) {
      case "embed-file": {
        const file = args[1];
        if (!file) { console.log("Usage: npm run db embed-file <file.md>"); break; }
        const fullPath = path.join(INPUT_PATH, file);
        const content = await fs.readFile(fullPath, "utf-8");
        await db.addMdFile(file, content, embedder);
        console.log(`✅ Indexed: ${file}`);
        break;
      }
      case "embed-folder": {
        const folder = args[1] || "";
        const folderAbs = path.join(INPUT_PATH, folder);
        let mdFiles: string[];
        try { mdFiles = await collectMdFiles(folderAbs); } catch {
          console.error(`❌ Folder not found: ${folderAbs}`);
          break;
        }
        if (mdFiles.length === 0) { console.log("No .md files found."); break; }
        console.log(`Found ${mdFiles.length} file(s)...`);
        const relFiles: string[] = [];
        const contents: string[] = [];
        for (const f of mdFiles) {
          try {
            contents.push(await fs.readFile(f, "utf-8"));
            relFiles.push(path.relative(INPUT_PATH, f));
          } catch {}
        }
        await db.addMdFiles(relFiles, contents, embedder);
        console.log(`✅ Indexed ${relFiles.length} file(s).`);
        break;
      }
      case "reindex-all": {
        const force = args[1] === "--force";
        await db.reindexAllFiles(
          embedder,
          async () => (await collectMdFiles(INPUT_PATH)).map(f => path.relative(INPUT_PATH, f)),
          async (rel) => fs.readFile(path.join(INPUT_PATH, rel), "utf-8"),
          force,
        );
        console.log(`✅ Reindexed all (force=${force}).`);
        break;
      }
      case "reconcile": {
        await db.reconcile(
          embedder,
          async () => (await collectMdFiles(INPUT_PATH)).map(f => path.relative(INPUT_PATH, f)),
          async (rel) => fs.readFile(path.join(INPUT_PATH, rel), "utf-8"),
        );
        console.log("✅ Reconciled.");
        break;
      }
      case "list": {
        const all = await db.getAllChunks();
        if (all.length === 0) { console.log("No chunks in DB."); break; }
        console.log(`Total chunks: ${all.length}\n`);
        all.forEach(c => printChunk({ filePath: c.filePath, chunkIndex: c.chunkIndex, chunkText: c.chunkText, tokenCount: c.tokenCount, embedding: c.embedding }));
        break;
      }
      case "get": {
        const query = args[1];
        if (!query) { console.log("Usage: npm run db get <file> [--by path|name]"); break; }
        const byIdx = args.indexOf("--by");
        const by = (byIdx >= 0 ? args[byIdx + 1] : undefined) as "path" | "name" | undefined;
        if (by) {
          const chunks = by === "path"
            ? await db.getFileChunks(query)
            : await db.getChunksByFileName(query);
          console.log(`Chunks: ${chunks.length}`);
          chunks.forEach(c => printChunk({ filePath: (c as any).filePath ?? query, chunkIndex: c.chunkIndex, chunkText: c.chunkText, tokenCount: c.tokenCount, embedding: c.embedding }));
        } else {
          const chunks = await db.getChunksByFileOrName(query);
          if (chunks.length === 0) { console.log(`No chunks for "${query}".`); break; }
          console.log(`Chunks: ${chunks.length}`);
          chunks.forEach(c => printChunk({ filePath: c.filePath ?? query, chunkIndex: c.chunkIndex, chunkText: c.chunkText, tokenCount: c.tokenCount, embedding: c.embedding }));
        }
        break;
      }
      case "soft-delete": {
        const file = args[1];
        if (!file) { console.log("Usage: npm run db soft-delete <file.md>"); break; }
        await db.softDeleteFile(file);
        console.log(`✅ Soft-deleted: ${file}`);
        break;
      }
      case "restore": {
        const file = args[1];
        if (!file) { console.log("Usage: npm run db restore <file.md>"); break; }
        await db.restoreFile(file);
        console.log(`✅ Restored: ${file}`);
        break;
      }
      case "hard-delete": {
        const file = args[1];
        if (!file) { console.log("Usage: npm run db hard-delete <file.md>"); break; }
        await db.hardDeleteFile(file);
        console.log(`✅ Hard-deleted: ${file}`);
        break;
      }
      case "rename": {
        const old = args[1], newP = args[2];
        if (!old || !newP) { console.log("Usage: npm run db rename <old> <new>"); break; }
        const ok = await db.renameFile(old, newP);
        console.log(ok ? `✅ Renamed ${old} → ${newP}` : `❌ ${old} not found`);
        break;
      }
      case "search": {
        const name = args[1];
        const limit = args[2] ? parseInt(args[2]) : undefined;
        const result = await db.search({ fileName: name, limit });
        if (result.length === 0) { console.log(`No results for "${name || "all"}".`); break; }
        console.log(`Results: ${result.length}`);
        result.forEach(r => printChunk({ filePath: r.filePath, chunkIndex: r.chunkIndex, chunkText: r.chunkText, tokenCount: r.tokenCount, embedding: r.embedding }));
        break;
      }
      case "optimize": {
        await db.optimize();
        console.log("✅ Optimized.");
        break;
      }
      case "force-vacuum": {
        await db.forceVacuum();
        console.log("✅ VACUUM complete.");
        break;
      }
      case "likeness": {
        const queryText = args[1];
        if (!queryText) { console.log("Usage: npm run db likeness <query> [--topK N] [--folder path] [--file path]"); break; }
        const topK = parseInt(args[args.indexOf("--topK") + 1] || "10") || 10;
        const folderIdx = args.indexOf("--folder");
        const folder = folderIdx >= 0 ? args[folderIdx + 1] : undefined;
        const fileIdx = args.indexOf("--file");
        const filePath = fileIdx >= 0 ? args[fileIdx + 1] : undefined;

        const start = performance.now();
        const results = await db.queryWithScores(embedder, queryText, {
          topK,
          filters: { filePath, folder },
        });
        const elapsed = (performance.now() - start).toFixed(0);

        if (results.length === 0) {
          console.log(`No results for "${queryText}". (${elapsed}ms)`);
          break;
        }
        console.log(`🔍 "${queryText}" — ${results.length} results (${elapsed}ms)\n`);
        results.forEach((r, i) => {
          const bar = "█".repeat(Math.round(r.score * 50));
          console.log(`#${i + 1}  [${r.score.toFixed(4)}] ${bar}`);
          console.log(`     📄 ${r.filePath}#${r.chunkIndex}`);
          if (r.chunkText) console.log(`     ${r.chunkText.slice(0, 150)}${r.chunkText.length > 150 ? "..." : ""}`);
          console.log();
        });
        break;
      }
      case "pipeline-search": {
        const queryText = args[1];
        if (!queryText) {
          console.log("Usage: npm run db pipeline-search <query> [--topK N] [--translate] [--synonyms static|llm] [--hyde] [--rerank]");
          break;
        }

        // Lazy-init LLM + Translator (expensive, only on first use)
        if (!llm) {
          const llmConfig = { backend: "ollama", model: "llama3.2:3b", temperature: 0.3 } as any;
          llm = new LLMService(llmConfig);
          console.log(`🤖 LLM ready: ${llmConfig.model}`);
        }
        if (!translator && (args.includes("--translate") || args.includes("--synonyms"))) {
          translator = new Translator(llm!, TRANSLATOR_CONFIG.targetLang);
          console.log(`🌐 Translator ready: →${TRANSLATOR_CONFIG.targetLang}`);
        }

        const topK = parseInt(args[args.indexOf("--topK") + 1] || "10") || 10;
        const autoTranslate = args.includes("--translate");
        const synonymsIdx = args.indexOf("--synonyms");
        const expandSynonyms = synonymsIdx >= 0 ? (args[synonymsIdx + 1] as "static" | "llm") : undefined;
        const hyde = args.includes("--hyde");
        const rerank = args.includes("--rerank");
        const enableLog = args.includes("--log");
        const logSinks: string[] = enableLog ? ["console", "file"] : [];
        if (enableLog && args.includes("--log-memory")) logSinks.push("memory");

        const config: any = {
          mode: "hybrid",
          pipeline: {
            autoTranslate,
            expandSynonyms: expandSynonyms ?? "off",
            hyde,
          },
          aiRerank: rerank ? { enabled: true, mode: "scoring", maxCandidates: 25 } : undefined,
          logging: enableLog ? { enabled: true, sinks: logSinks as any, fileSink: { dir: "src/test/test-db/traces" } } : undefined,
        };

        console.log(`\n⚙️  Pipeline: translate=${autoTranslate} synonyms=${expandSynonyms ?? "off"} hyde=${hyde} rerank=${rerank}`);
        console.log(`🔍 Query: "${queryText}"\n`);

        const t0 = performance.now();
        const { results, elapsed } = await db.queryPipeline(
          embedder,
          queryText,
          config,
          translator,
          rerank || expandSynonyms === "llm" || hyde ? llm : undefined,
          { topK },
        );

        if (results.length === 0) {
          console.log(`No results. (${elapsed.toFixed(0)}ms)`);
          break;
        }

        // Merge chunks into file-level results
        const files = mergeByFile(
          results.map(r => ({
            chunkRef: { filePath: r.filePath, chunkIndex: r.chunkIndex },
            score: r.score,
            text: r.chunkText,
          })),
        );

        console.log(`📊 ${results.length} chunks → ${files.length} files in ${elapsed.toFixed(0)}ms\n`);
        // Normalize scores so top = 1.00 (relative relevance)
        const maxScore = files[0]?.score ?? 1;
        files.slice(0, topK).forEach((r, i) => {
          const normalized = maxScore > 0 ? r.score / maxScore : 0;
          const bar = "█".repeat(Math.min(Math.round(normalized * 80), 80));
          console.log(`#${i + 1}  [${normalized.toFixed(2)}] ${bar}`);
          console.log(`     📄 ${r.filePath}  (${r.chunkCount} chunks)`);
          if (r.topChunk.text) console.log(`     ${r.topChunk.text.slice(0, 180)}${r.topChunk.text.length > 180 ? "..." : ""}`);
          console.log();
        });
        break;
      }
      case "info": {
        const exists = await fs.access(dbFile).then(() => true).catch(() => false);
        console.log(`DB file: ${dbFile}`);
        console.log(`Exists: ${exists}`);
        if (exists) {
          const stat = await fs.stat(dbFile);
          console.log(`Size: ${(stat.size / 1024).toFixed(1)} KB`);
          console.log(`Modified: ${stat.mtime}`);
        }
        const all = await db.getAllChunks();
        console.log(`Chunks: ${all.length}`);
        const files = new Set(all.map(c => c.filePath));
        console.log(`Files: ${files.size}`);
        break;
      }
      default:
        console.log(`
Commands:
  npm run db embed-file <file>     Index one file
  npm run db embed-folder [dir]    Index all .md in folder
  npm run db reindex-all [--force] Reindex entire vault
  npm run db reconcile             Sync DB with vault
  npm run db likeness <query>       Semantic search ranked by score
         [--topK N] [--folder path] [--file path]
  npm run db pipeline-search <q>     Full pipeline: translate, synonyms, hyde, rerank
         [--topK N] [--translate] [--synonyms static|llm] [--hyde] [--rerank] [--log]
  npm run db list                  Show all chunks
  npm run db get <file>            Show chunks for file
  npm run db search [name] [limit] Query API search
  npm run db soft-delete <file>    Mark as deleted
  npm run db restore <file>        Undo soft delete
  npm run db hard-delete <file>    Permanently remove
  npm run db rename <old> <new>    Rename file
  npm run db optimize              PRAGMA optimize
  npm run db force-vacuum          VACUUM
  npm run db info                  DB file path + stats
`);
    }
  } catch (err) {
    console.error("❌", err);
  }
  await db.close();
}

main().catch(console.error);

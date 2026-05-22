import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/refinery/integration/AnkiRefineryPipeline.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { RefineryService } from "@refinery/RefineryService";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";
import { fakeFs } from "../helpers/fakeFs";
import { nullLogger } from "../helpers/nullLogger";

const sampleMd = `# Topic
Paragraph one with enough content to chunk.

## Sub-topic
Paragraph two has different content for testing.`;

describe("AnkiRefineryPipeline Integration", () => {
  let adapter: FakeDataAdapter;
  let dbConnection: TauriDbConnection;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    dbConnection = new TauriDbConnection(adapter as any, "anki-refinery-test", "test-model");
    const schema = new SchemaManager(dbConnection);
    await schema.initialize();
  });

  afterEach(async () => {
    try {
      await dbConnection.close();
    } catch {}
  });

  const makeService = (fileContents: Record<string, string>, opts: any = {}) => {
    const fileAccess = fakeFs(fileContents);
    const llm = {
      generateMessages: async (messages: any[]) => {
        const sys = messages.find(m => m.role === "system")?.content ?? "";
        if (sys.includes("atomic flashcard") || sys.includes("question-style heading")) {
          // Return heading based on chunk content
          const userMsg = messages.find(m => m.role === "user")?.content ?? "";
          if (userMsg.includes("Sub-topic") || userMsg.includes("two")) {
            return { text: '{"questionHeading":"## What is Subtopic?","filename":"what-is-subtopic.md"}' };
          }
          return { text: '{"questionHeading":"## What is Topic?","filename":"what-is-topic.md"}' };
        }
        if (sys.includes("aliases") || sys.includes("synonyms")) {
          return { text: '["test-alias"]' };
        }
        if (sys.includes("tags") || sys.includes("keywords")) {
          return { text: '["test-tag"]' };
        }
        if (sys.includes("roots") && !sys.includes("foldersToCreate")) {
          return { text: '{"roots":[{"name":"biology","children":[],"assignedChunks":[]}], "rationale":"auto"}' };
        }
        if (sys.includes("foldersToCreate")) {
          const userMsg = messages.find(m => m.role === "user")?.content ?? "";
          const hashes = [...userMsg.matchAll(/<chunk hash="([^"]+)">/g)].map(m => m[1]);
          const decisions = hashes.map((hash, i) => {
            const filename = i === 1 ? "what-is-subtopic.md" : "what-is-topic.md";
            return {
              chunkHash: hash,
              action: opts.actionOverride ?? "create",
              outputPath: `biology/${filename}`,
              existingTarget: opts.actionOverride === "merge" ? `biology/${filename}` : undefined
            };
          });
          return { text: JSON.stringify({ decisions, foldersToCreate: ["biology"], rationale: "auto" }) };
        }
        if (sys.includes("merge two similar")) {
          return { text: "## What is Topic?\n\nMerged content via LLM." };
        }
        return { text: "{}" };
      },
    } as any;

    return new RefineryService({
      fileAccess,
      retrieval: { query: async () => [], setEmbedder: () => {}, markDirty: () => {} } as any,
      llm,
      logger: nullLogger(),
      outputRoot: "output",
      inboxRoot: "inbox",
      dryRun: false,
      dbConnection,
      ...opts,
    });
  };

  it("preserves SRS statistics during card update/merge", async () => {
    // 1. First run: create a card
    const svc = makeService({ "inbox/note.md": "Paragraph one with enough content to chunk." }, { overwriteOnMerge: true, actionOverride: "create" });
    const result = await svc.refine("note.md");
    console.log("REFINE RESULT CHUNKS:", JSON.stringify(result.chunks));
    console.log("REFINE OPERATIONS:", JSON.stringify(result.operations));
    console.log("PLACEMENT PLAN:", JSON.stringify(result.placement));
    console.log("FILESYSTEM FILES:", JSON.stringify(Array.from((svc.ctx.fileAccess as any).files.keys())));

    // Check card exists in DB
    const filesDb = await dbConnection.db!.exec("SELECT file_path, file_id, source_file_id FROM files");
    console.log("DB FILES:", JSON.stringify(filesDb));
    const cardsDb = await dbConnection.db!.exec("SELECT file_id FROM cards");
    console.log("DB CARDS:", JSON.stringify(cardsDb));

    const selectCard = () => await dbConnection.db!.exec("SELECT c.reps, c.ease_factor, f.file_path FROM cards c JOIN files f ON c.file_id = f.file_id WHERE f.file_path = 'output/biology/what-is-topic.md'");
    const before = selectCard();
    assert.strictEqual(before.length, 1);
    assert.strictEqual(before[0].values[0][0], 0, "reps should start at 0");
    assert.strictEqual(before[0].values[0][1], 2.5, "ease factor should start at 2.5");

    // 2. Modify SRS statistics manually in DB
    await dbConnection.db!.run("UPDATE cards SET reps = 7, ease_factor = 2.8 WHERE file_id = (SELECT file_id FROM files WHERE file_path = 'output/biology/what-is-topic.md')");

    // 3. Second run: update card (via merge/overwrite)
    // Setup retrieval to return the card as similar to trigger merge action
    const svc2 = makeService({ "inbox/note.md": "Paragraph one with enough content to chunk." }, { overwriteOnMerge: true, actionOverride: "merge" });
    (svc2 as any).ctx.retrieval = {
      query: async () => [{
        score: 0.90,
        text: "Paragraph one with enough content to chunk.",
        chunkRef: { filePath: "output/biology/what-is-topic.md" }
      }],
      setEmbedder: () => {},
      markDirty: () => {}
    } as any;

    const result2 = await svc2.refine("note.md");
    console.log("SECOND RUN OPERATIONS:", JSON.stringify(result2.operations));
    console.log("SECOND RUN PLACEMENT PLAN:", JSON.stringify(result2.placement));

    // Verify SRS statistics are preserved!
    const after = selectCard();
    assert.strictEqual(after.length, 1);
    assert.strictEqual(after[0].values[0][0], 7, "reps should be preserved as 7");
    assert.strictEqual(after[0].values[0][1], 2.8, "ease factor should be preserved as 2.8");
  });

  it("garbage collects deleted card files from disk and database (deleteFromDbOnGc = true)", async () => {
    // 1. Run pipeline with 2 chunks to create card A and card B
    const svc = makeService({
      "inbox/note.md": `Paragraph one with enough content to chunk.

## Sub-topic
Paragraph two has different content for testing.`
    }, { deleteFromDbOnGc: true, actionOverride: "create" });

    await svc.refine("note.md");

    // Verify both files written to disk
    assert.ok(await svc.ctx.fileAccess.exists("output/biology/what-is-topic.md"));
    assert.ok(await svc.ctx.fileAccess.exists("output/biology/what-is-subtopic.md"));

    // Verify both files indexed in DB
    const countFiles = () => await dbConnection.db!.exec("SELECT COUNT(*) FROM files WHERE source_file_id IS NOT NULL AND is_deleted = 0")[0].values[0][0];
    assert.strictEqual(countFiles(), 2);

    // 2. Change note.md to contain only 1 chunk, run pipeline again
    // Re-instantiate service to load updated fake file content
    const svc2 = makeService({
      "inbox/note.md": `Paragraph one with enough content to chunk.`
    }, { deleteFromDbOnGc: true, actionOverride: "create" });

    await svc2.refine("note.md");

    // Verify card B (what-is-subtopic.md) is deleted from disk
    assert.ok(await svc2.ctx.fileAccess.exists("output/biology/what-is-topic.md"));
    assert.strictEqual(await svc2.ctx.fileAccess.exists("output/biology/what-is-subtopic.md"), false);

    // Verify only card A remains in DB (card B deleted due to deleteFromDbOnGc = true)
    assert.strictEqual(countFiles(), 1);
    const dbRows = await dbConnection.db!.exec("SELECT file_path FROM files WHERE source_file_id IS NOT NULL");
    assert.strictEqual(dbRows[0].values[0][0], "output/biology/what-is-topic.md");
  });

  it("garbage collects deleted card files from disk and soft-deletes in DB (deleteFromDbOnGc = false)", async () => {
    // 1. Run pipeline with 2 chunks to create card A and card B
    const svc = makeService({
      "inbox/note.md": `Paragraph one with enough content to chunk.

## Sub-topic
Paragraph two has different content for testing.`
    }, { deleteFromDbOnGc: false, actionOverride: "create" });

    await svc.refine("note.md");

    // Verify both files written to disk
    assert.ok(await svc.ctx.fileAccess.exists("output/biology/what-is-topic.md"));
    assert.ok(await svc.ctx.fileAccess.exists("output/biology/what-is-subtopic.md"));

    // 2. Change note.md to contain only 1 chunk, run pipeline again
    const svc2 = makeService({
      "inbox/note.md": `Paragraph one with enough content to chunk.`
    }, { deleteFromDbOnGc: false, actionOverride: "create" });

    await svc2.refine("note.md");

    // Verify card B is deleted from disk
    assert.strictEqual(await svc2.ctx.fileAccess.exists("output/biology/what-is-subtopic.md"), false);

    // Verify card B still exists in DB but is marked is_deleted = 1
    const countActive = await dbConnection.db!.exec("SELECT COUNT(*) FROM files WHERE source_file_id IS NOT NULL AND is_deleted = 0")[0].values[0][0];
    const countAll = await dbConnection.db!.exec("SELECT COUNT(*) FROM files WHERE source_file_id IS NOT NULL")[0].values[0][0];
    
    assert.strictEqual(countActive, 1);
    assert.strictEqual(countAll, 2);

    const deletedRow = await dbConnection.db!.exec("SELECT is_deleted FROM files WHERE file_path = 'output/biology/what-is-subtopic.md'");
    assert.strictEqual(deletedRow[0].values[0][0], 1);
  });
});

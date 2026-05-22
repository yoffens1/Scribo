// src/test/refinery/unit/PlacementPlanner.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { PlacementPlanner } from "@refinery/placement/PlacementPlanner";
import type { ChunkWithHash } from "@refinery/types/chunk-decision";
import { nullLogger } from "../helpers/nullLogger";

const emptyTaxonomy = { roots: [], rationale: "" };
const chunk: ChunkWithHash = { hash: "h1", embeddingText: "test content", generationText: "test content", text: "test content", index: 0, sourcePath: "inbox/test.md" };

describe("PlacementPlanner.plan", () => {
  it("returns empty plan for empty chunks", async () => {
    const planner = new PlacementPlanner({} as any, nullLogger());
    const plan = await planner.plan(emptyTaxonomy, "", []);
    assert.strictEqual(plan.decisions.length, 0);
    assert.strictEqual(plan.foldersToCreate.length, 0);
    assert.strictEqual(plan.rationale, "no chunks to place");
  });

  it("parses valid LLM JSON response", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '{"decisions":[{"chunkHash":"h1","outputPath":"ai/ml.md","action":"create","reason":"new"}],"foldersToCreate":["ai"],"rationale":"test placement"}',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    const plan = await planner.plan({ roots: [], rationale: "" }, "/\n  ai/", [chunk]);
    assert.strictEqual(plan.decisions.length, 1);
    assert.strictEqual(plan.decisions[0].chunkHash, "h1");
    assert.strictEqual(plan.decisions[0].outputPath, "ai/ml.md");
    assert.strictEqual(plan.decisions[0].action, "create");
    assert.strictEqual(plan.foldersToCreate.length, 1);
    assert.strictEqual(plan.foldersToCreate[0], "ai");
  });

  it("throws on malformed JSON", async () => {
    const llm = {
      generateMessages: async () => ({ text: "not valid json at all" }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    await assert.rejects(
      () => planner.plan({ roots: [], rationale: "" }, "", [chunk]),
      /PlacementPlanner.*valid JSON/,
    );
  });

  it("throws on empty LLM response", async () => {
    const llm = {
      generateMessages: async () => ({ text: "" }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    await assert.rejects(
      () => planner.plan({ roots: [], rationale: "" }, "", [chunk]),
      /PlacementPlanner.*valid JSON/,
    );
  });

  it("parses JSON wrapped in markdown code fences", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '```json\n{"decisions":[{"chunkHash":"h1","outputPath":"x.md","action":"create","reason":"r"}],"foldersToCreate":[],"rationale":"r"}\n```',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    const plan = await planner.plan({ roots: [], rationale: "" }, "", [chunk]);
    assert.strictEqual(plan.decisions.length, 1);
    assert.strictEqual(plan.decisions[0].outputPath, "x.md");
  });

  it("handles JSON with extra whitespace", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '  \n  \n{"decisions": [], "foldersToCreate": [], "rationale": "clean"}\n  ',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    const plan = await planner.plan({ roots: [], rationale: "" }, "", [chunk]);
    assert.strictEqual(plan.rationale, "clean");
  });
});

describe("LLM JSON parsing edge cases", () => {
  it("handles partial JSON gracefully", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '{"decisions": [{"chunkHash":"h1","outputPath":"x.md"',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    await assert.rejects(
      () => planner.plan({ roots: [], rationale: "" }, "", [chunk]),
    );
  });

  it("handles multiple JSON objects (picks first valid)", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '{"decisions":[],"foldersToCreate":[],"rationale":"first"}\n{"decisions":[],"foldersToCreate":[],"rationale":"second"}',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    const plan = await planner.plan({ roots: [], rationale: "" }, "", [chunk]);
    assert.strictEqual(plan.rationale, "first");
  });

  it("handles trailing text after JSON", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '{"decisions":[],"foldersToCreate":[],"rationale":"ok"} some extra text here',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    const plan = await planner.plan({ roots: [], rationale: "" }, "", [chunk]);
    assert.strictEqual(plan.rationale, "ok");
  });

  it("handles invalid values in JSON", async () => {
    const llm = {
      generateMessages: async () => ({
        text: '{"decisions":undefined,"foldersToCreate":[],"rationale":"bad"}',
      }),
    } as any;
    const planner = new PlacementPlanner(llm, nullLogger());
    await assert.rejects(
      () => planner.plan({ roots: [], rationale: "" }, "", [chunk]),
    );
  });
});

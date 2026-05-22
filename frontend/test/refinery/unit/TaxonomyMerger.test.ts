// src/test/refinery/unit/TaxonomyMerger.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { TaxonomyMerger } from "@refinery/taxonomy/TaxonomyMerger";
import { nullLogger } from "../helpers/nullLogger";

const callFindBestMatch = (m: TaxonomyMerger, proposed: string, existing: string[]): string | null =>
  (m as any).findBestMatch(proposed, existing);

describe("TaxonomyMerger.findBestMatch", () => {
  const m = new TaxonomyMerger(nullLogger());

  it("matches exact case-insensitively", async () => {
    assert.strictEqual(
      callFindBestMatch(m, "Machine-Learning", ["ai", "machine-learning", "philosophy"]),
      "machine-learning",
    );
  });

  it("matches suffix path (ends with /proposed)", async () => {
    assert.strictEqual(
      callFindBestMatch(m, "ml", ["notes", "ai/ml", "programming"]),
      "ai/ml",
    );
  });

  it("returns null for no match", async () => {
    assert.strictEqual(
      callFindBestMatch(m, "quantum", ["ai", "programming"]),
      null,
    );
  });

  it("levenshtein match — proposed is very similar to existing", async () => {
    // "machine-learn" is very close to "machine-learning" (only missing 3 chars)
    // distance is 3, maxLength is 16 -> sim = 13/16 = 0.8125 > 0.80 -> MATCH
    assert.strictEqual(
      callFindBestMatch(m, "machine-learn", ["ai/machine-learning", "programming"]),
      "ai/machine-learning",
    );
  });

  it("levenshtein match — rejected if similarity is too low", async () => {
    // "machine" vs "machine-learning"
    // distance is 9, max is 16 -> sim = 7/16 = 0.4375 < 0.80 -> NO MATCH
    assert.strictEqual(
      callFindBestMatch(m, "machine", ["machine-learning"]),
      null,
    );
  });

  it("handles empty existing array", async () => {
    assert.strictEqual(callFindBestMatch(m, "anything", []), null);
  });

  it("exact match preferred over levenshtein (first in order)", async () => {
    assert.strictEqual(
      callFindBestMatch(m, "go", ["go", "golang", "gopher-lang"]),
      "go",
    );
  });

  it("BUG FIXED: greedy contains match no longer triggers — 'go' vs 'argo'", async () => {
    const result = callFindBestMatch(m, "go", ["argo/notes"]);
    // Levenshtein similarity between "go" and "argo/notes" or "notes" is very low.
    assert.strictEqual(result, null,
      "contains match is no longer greedy — 'go' should not match 'argo/notes'");
  });

  it("BUG FIXED: 'go' vs 'django' no longer triggers", async () => {
    const result = callFindBestMatch(m, "go", ["python/django"]);
    assert.strictEqual(result, null,
      "'django'.includes('go') no longer causes false positive");
  });

  it("suffix match preferred over levenshtein", async () => {
    // "ml" matches "ai/ml" by suffix BEFORE trying fuzzy match against "html"
    assert.strictEqual(
      callFindBestMatch(m, "ml", ["ai/ml", "html"]),
      "ai/ml",
    );
  });
});

describe("TaxonomyMerger.match", () => {
  const m = new TaxonomyMerger(nullLogger());

  it("maps proposed paths to existing folders", async () => {
    const mapping = m.match(
      {
        roots: [
          { name: "ai", description: "", children: [], assignedChunks: [] },
          { name: "new-topic", description: "", children: [], assignedChunks: [] },
        ],
        rationale: "",
      },
      ["ai", "programming"],
    );
    assert.strictEqual(mapping.get("ai"), "ai");
    assert.strictEqual(mapping.get("new-topic"), null);
  });

  it("handles nested taxonomy nodes", async () => {
    const mapping = m.match(
      {
        roots: [
          {
            name: "programming",
            description: "",
            children: [
              { name: "rust", description: "", children: [], assignedChunks: [] },
            ],
            assignedChunks: [],
          },
        ],
        rationale: "",
      },
      ["programming", "programming/rust", "programming/python"],
    );
    assert.strictEqual(mapping.get("programming"), "programming");
    assert.strictEqual(mapping.get("programming/rust"), "programming/rust");
  });

  it("handles empty taxonomy", async () => {
    const mapping = m.match({ roots: [], rationale: "" }, ["ai"]);
    assert.strictEqual(mapping.size, 0);
  });
});

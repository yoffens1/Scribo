import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  HttpError, AIError, ValidationError, ParseError, ConfigurationError,
} from "@ai/transport/HttpError";

describe("HttpError", () => {
  it("stores fields", async () => {
    const e = new HttpError("msg", 429, "openai", "rate limited");
    assert.equal(e.status, 429);
    assert.equal(e.provider, "openai");
    assert.equal(e.body, "rate limited");
    assert.equal(e.name, "HttpError");
    assert.ok(e instanceof Error);
  });
  it("body optional", async () => {
    assert.equal(new HttpError("msg", 500, "g").body, undefined);
  });
});

describe("AIError", () => {
  it("stores provider + cause", async () => {
    const cause = new Error("inner");
    const e = new AIError("fail", "ollama", cause);
    assert.equal(e.provider, "ollama");
    assert.equal(e.cause, cause);
  });
});

describe("ValidationError", () => {
  it("extends AIError", async () => {
    const e = new ValidationError("bad", "o");
    assert.ok(e instanceof AIError);
    assert.equal(e.name, "ValidationError");
  });
});

describe("ParseError", () => {
  it("extends AIError", async () => {
    const e = new ParseError("bad json", "g");
    assert.ok(e instanceof AIError);
    assert.equal(e.name, "ParseError");
  });
});

describe("ConfigurationError", () => {
  it("not AIError", async () => {
    const e = new ConfigurationError("key required");
    assert.ok(e instanceof Error);
    assert.equal(e instanceof AIError, false);
    assert.equal(e.name, "ConfigurationError");
  });
});

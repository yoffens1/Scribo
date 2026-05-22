// src/test/refinery/unit/TransactionalWriter.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { TransactionalWriter } from "@refinery/writers/TransactionalWriter";
import { FileWriter } from "@refinery/writers/FileWriter";
import type { WriteOperation } from "@refinery/types/refinery-result";
import { nullLogger, spyLogger } from "../helpers/nullLogger";
import { fakeFs } from "../helpers/fakeFs";

const op = (type: WriteOperation["type"], extra = {}): WriteOperation =>
  ({ type, ...extra } as WriteOperation);

describe("TransactionalWriter", () => {
  it("executes all operations in order on success", async () => {
    const executed: number[] = [];
    const writer = {
      execute: async (o: WriteOperation & { _id: number }) => { executed.push(o._id); },
    } as any;
    const tx = new TransactionalWriter(writer, fakeFs(), nullLogger());

    await tx.executeBatch([
      op("create_file", { _id: 1 }),
      op("create_file", { _id: 2 }),
      op("create_folder", { _id: 3 }),
    ]);

    assert.deepStrictEqual(executed, [1, 2, 3]);
  });

  it("rolls back executed ops in reverse order on failure", async () => {
    const executed: number[] = [];
    const rolledBack: number[] = [];
    const writer = {
      execute: async (o: WriteOperation & { _id: number }) => {
        if (o._id === 3) throw new Error("fail at op 3");
        executed.push(o._id);
      },
    } as any;

    // Mock rollbackOp to track calls
    const tx = new TransactionalWriter(writer, fakeFs(), nullLogger());
    const origRollback = (tx as any).rollbackOp.bind(tx);
    (tx as any).rollbackOp = async (o: WriteOperation & { _id: number }) => {
      rolledBack.push(o._id);
      return origRollback(o);
    };

    await assert.rejects(() => tx.executeBatch([
      op("create_file", { _id: 1 }),
      op("create_file", { _id: 2 }),
      op("create_file", { _id: 3 }), // fails here
      op("create_folder", { _id: 4 }),
    ]));

    assert.deepStrictEqual(executed, [1, 2]);
    // rollback in reverse: [2, 1]
    assert.deepStrictEqual(rolledBack, [2, 1]);
  });

  it("rollback failures do not throw (best-effort)", async () => {
    const writer = {
      execute: async (o: WriteOperation & { _id: number }) => {
        if (o._id === 2) throw new Error("fail");
      },
    } as any;

    const tx = new TransactionalWriter(writer, fakeFs(), nullLogger());
    (tx as any).rollbackOp = async () => { throw new Error("rollback also fails"); };

    // Should still throw the original error, not the rollback error
    await assert.rejects(
      () => tx.executeBatch([
        op("create_file", { _id: 1 }),
        op("create_file", { _id: 2 }),
      ]),
      /fail/,
    );
  });

  it("handles empty operation batch", async () => {
    const executed: number[] = [];
    const writer = {
      execute: async () => { executed.push(1); },
    } as any;
    const tx = new TransactionalWriter(writer, fakeFs(), nullLogger());

    await tx.executeBatch([]);
    assert.strictEqual(executed.length, 0);
  });

  it("resets executed list between batches", async () => {
    const executed: any[] = [];
    let batchCount = 0;
    const writer = {
      execute: async (o: WriteOperation & { fail?: boolean }) => {
        if (o.fail) throw new Error("batch 2 fail");
        executed.push(o.type);
      },
    } as any;
    const tx = new TransactionalWriter(writer, fakeFs(), nullLogger());

    // Batch 1 succeeds
    await tx.executeBatch([op("create_folder")]);
    assert.strictEqual(executed.length, 1);
    executed.length = 0;

    // Batch 2: op1 succeeds, op2 fails
    await assert.rejects(() =>
      tx.executeBatch([
        { type: "create_file" } as WriteOperation & { fail?: boolean },
        { type: "create_file", fail: true } as WriteOperation & { fail?: boolean },
      ])
    );
    // Only op1 from batch 2 was executed before failure
    assert.strictEqual(executed.length, 1);
  });
});

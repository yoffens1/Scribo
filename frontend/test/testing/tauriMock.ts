// src/test/testing/tauriMock.ts
import * as fs from "fs";

if (typeof window === "undefined") {
  (global as any).window = {};
}

if (!(global.window as any).__TAURI_INTERNALS__) {
  console.log("Mocking Tauri IPC for Node.js environment...");

  const mockInvoke = async (command: string, args: any = {}) => {
    switch (command) {
      case "fs_read_text":
        return fs.readFileSync(args.path, "utf-8");
      case "fs_read_binary":
        const buf = fs.readFileSync(args.path);
        return Array.from(buf);
      case "fs_write_text":
        fs.writeFileSync(args.path, args.content, "utf-8");
        return;
      case "fs_exists":
        return fs.existsSync(args.path);
      case "fs_list":
        const entries = fs.readdirSync(args.path, { withFileTypes: true });
        return entries.map(e => ({ name: e.name, is_dir: e.isDirectory() }));
      case "fs_rename":
        fs.renameSync(args.from, args.to);
        return;
      case "fs_delete":
        const stat = fs.statSync(args.path);
        if (stat.isDirectory()) {
          fs.rmSync(args.path, { recursive: true, force: true });
        } else {
          fs.unlinkSync(args.path);
        }
        return;
      
      case "db_initialize":
        return;
      case "db_close":
        return;
      case "db_execute":
        return 0;
      case "db_select":
        return [];
      default:
        throw new Error(`Unhandled mock command: ${command}`);
    }
  };

  (global.window as any).__TAURI_INTERNALS__ = { invoke: mockInvoke };
}

export const initMock = true;


export class FakeDataAdapter   {
  private store: Map<string, ArrayBuffer> = new Map();

  async readBinary(normalizedPath: string): Promise<ArrayBuffer> {
    const data = this.store.get(normalizedPath);
    if (!data) throw new Error("File not found");
    return data;
  }

  async writeBinary(normalizedPath: string, data: ArrayBuffer): Promise<void> {
    this.store.set(normalizedPath, data);
  }

  async exists(normalizedPath: string): Promise<boolean> {
    return this.store.has(normalizedPath);
  }

  async remove(normalizedPath: string): Promise<void> {
    this.store.delete(normalizedPath);
  }

  getName(): string {
    return "fake-vault";
  }

  stat(normalizedPath: string): Promise<any> { throw new Error("Not implemented"); }
  append(normalizedPath: string, data: string): Promise<void> { throw new Error("Not implemented"); }
  appendBinary(normalizedPath: string, data: ArrayBuffer): Promise<void> { throw new Error("Not implemented"); }
  process(normalizedPath: string, fn: (data: string) => string): Promise<string> { throw new Error("Not implemented"); }
  read(normalizedPath: string): Promise<string> { throw new Error("Not implemented"); }
  write(normalizedPath: string, data: string): Promise<void> { throw new Error("Not implemented"); }
  list(normalizedPath: string): Promise<any> { throw new Error("Not implemented"); }
  mkdir(normalizedPath: string): Promise<void> { throw new Error("Not implemented"); }
  rmdir(normalizedPath: string, recursive: boolean): Promise<void> { throw new Error("Not implemented"); }
  getResourcePath(normalizedPath: string): string { return normalizedPath; }
  trashSystem(normalizedPath: string): Promise<boolean> { throw new Error("Not implemented"); }
  trashLocal(normalizedPath: string): Promise<void> { throw new Error("Not implemented"); }
  rename(normalizedPath: string, normalizedNewPath: string): Promise<void> { throw new Error("Not implemented"); }
  copy(normalizedPath: string, normalizedNewPath: string): Promise<void> { throw new Error("Not implemented"); }
}

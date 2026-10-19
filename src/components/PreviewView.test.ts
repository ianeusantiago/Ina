import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: vi.fn(), invoke: vi.fn() }));
const { localUrl } = await import("./PreviewView");

describe("localUrl", () => {
  it("accepts only local http(s) addresses", () => {
    expect(localUrl("http://localhost:5173")).toBe("http://localhost:5173/");
    expect(localUrl(" http://127.0.0.1:3000/app ")).toBe("http://127.0.0.1:3000/app");
    expect(localUrl("https://example.com")).toBeNull();
    expect(localUrl("file:///C:/x/index.html")).toBeNull();
    expect(localUrl("javascript:alert(1)")).toBeNull();
    expect(localUrl("nada")).toBeNull();
  });
});

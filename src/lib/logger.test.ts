import { describe, it, expect, vi, beforeEach } from "vitest";
import { createLogger, setLogSession } from "../shared/lib/logger";

describe("logger", () => {
  beforeEach(() => {
    setLogSession(null);
  });

  it("createLogger returns an object with all log methods", () => {
    const log = createLogger("Test");
    expect(typeof log.debug).toBe("function");
    expect(typeof log.info).toBe("function");
    expect(typeof log.warn).toBe("function");
    expect(typeof log.error).toBe("function");
  });

  it("setLogSession does not throw", () => {
    expect(() => setLogSession("abc12345-6789")).not.toThrow();
    expect(() => setLogSession(null)).not.toThrow();
  });

  it("logger emits to console with tag prefix", () => {
    const spy = vi.spyOn(console, "info").mockImplementation(() => {});
    const log = createLogger("MyTag");

    log.info("hello");

    expect(spy).toHaveBeenCalledWith("[TYX:MyTag]", "hello");
    spy.mockRestore();
  });

  it("logger includes session id prefix when set", () => {
    const spy = vi.spyOn(console, "warn").mockImplementation(() => {});
    setLogSession("abcdef12-3456-7890");
    const log = createLogger("Tag");

    log.warn("test msg");

    expect(spy).toHaveBeenCalledWith("[TYX:Tag:abcdef12]", "test msg");
    spy.mockRestore();
  });

  it("logger passes data argument when provided", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    const log = createLogger("X");

    log.error("fail", { code: 42 });

    expect(spy).toHaveBeenCalledWith("[TYX:X]", "fail", { code: 42 });
    spy.mockRestore();
  });
});

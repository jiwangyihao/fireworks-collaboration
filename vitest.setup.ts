// CSS.escape polyfill for jsdom environment
if (typeof CSS === "undefined") {
  (globalThis as any).CSS = { escape: (s: string) => s };
} else if (typeof CSS.escape !== "function") {
  (CSS as any).escape = (s: string) => s;
}

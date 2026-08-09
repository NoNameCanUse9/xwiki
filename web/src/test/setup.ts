import "@testing-library/jest-dom/vitest";

// Node 26 在全局作用域暴露实验性 localStorage；未传 --localstorage-file 时
// 它是 undefined，会遮蔽 jsdom 提供的 window.localStorage。业务代码（文档
// 草稿持久化等）直接访问全局 localStorage，这里保证测试环境始终可用。
if (typeof globalThis.localStorage === "undefined") {
  const store = new Map<string, string>();
  globalThis.localStorage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key),
    clear: () => store.clear(),
    key: (index: number) => [...store.keys()][index] ?? null,
    get length() {
      return store.size;
    },
  } as Storage;
}

import { useState, useEffect, useCallback } from "react";

export default function DictionaryConfig() {
  const [words, setWords] = useState<string[]>([]);
  const [newWord, setNewWord] = useState("");
  const [loading, setLoading] = useState(true);

  // Load dictionary on mount.
  useEffect(() => {
    async function load() {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const dict = await invoke<string[]>("get_dictionary");
        setWords(dict);
      } catch {
        // Dev mode fallback
      }
      setLoading(false);
    }
    load();
  }, []);

  const handleAdd = useCallback(async () => {
    const trimmed = newWord.trim();
    if (!trimmed) return;
    if (words.includes(trimmed)) {
      setNewWord("");
      return;
    }
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("add_dictionary_word", { word: trimmed });
      setWords((prev) => [...prev, trimmed]);
    } catch {
      // Dev mode
    }
    setNewWord("");
  }, [newWord, words]);

  const handleRemove = useCallback(async (word: string) => {
    // 乐观更新：先移除，失败时回滚
    setWords((prev) => prev.filter((w) => w !== word));
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("remove_dictionary_word", { word });
    } catch {
      // 删除失败，回滚
      setWords((prev) => [...prev, word]);
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAdd();
      }
    },
    [handleAdd]
  );

  if (loading) {
    return <div className="text-gray-400 text-sm">加载词典中...</div>;
  }

  return (
    <div className="space-y-6 max-w-lg">
      <div>
        <h2 className="text-base font-medium text-gray-700">个人词典</h2>
        <p className="text-sm text-gray-500 mt-1">
          添加常用专有名词、术语、缩写，AI 润色时会优先使用这些词汇的正确写法。
        </p>
      </div>

      {/* Add word input */}
      <div className="flex gap-2">
        <input
          type="text"
          value={newWord}
          onChange={(e) => setNewWord(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="输入词汇，按回车添加"
          className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm
                     focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
        />
        <button
          onClick={handleAdd}
          disabled={!newWord.trim()}
          className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg
                     hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed
                     transition-colors"
        >
          添加
        </button>
      </div>

      {/* Word list */}
      {words.length > 0 ? (
        <div className="border border-gray-200 rounded-lg divide-y divide-gray-100">
          {words.map((word) => (
            <div
              key={word}
              className="flex items-center justify-between px-4 py-2.5"
            >
              <span className="text-sm text-gray-700">{word}</span>
              <button
                onClick={() => handleRemove(word)}
                className="text-gray-400 hover:text-red-500 text-sm transition-colors"
              >
                删除
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-sm text-gray-400 text-center py-8 border border-dashed border-gray-200 rounded-lg">
          尚未添加任何词汇
        </div>
      )}

      <p className="text-xs text-gray-400">
        已添加 {words.length} 个词汇
      </p>
    </div>
  );
}

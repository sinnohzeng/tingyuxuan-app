/**
 * 词典 CRUD hook。
 *
 * 封装词汇的加载、添加（乐观更新 + 失败回滚）、删除。
 * 从 DictionaryConfig.tsx 迁移核心逻辑。
 */
import { useState, useEffect, useCallback } from "react";
import { useUIStore } from "../../../shared/stores/uiStore";

export interface UseDictionaryReturn {
  words: string[];
  isLoading: boolean;
  addWord: (word: string) => Promise<void>;
  removeWord: (word: string) => Promise<void>;
}

export function useDictionary(): UseDictionaryReturn {
  const [words, setWords] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const dict = await invoke<string[]>("get_dictionary");
        setWords(dict);
      } catch (e) {
        console.error("[useDictionary] 加载词典失败:", e);
        useUIStore.getState().showToast({ type: "error", title: "加载词典失败" });
      }
      setIsLoading(false);
    })();
  }, []);

  const addWord = useCallback(
    async (word: string) => {
      const trimmed = word.trim();
      if (!trimmed || words.includes(trimmed)) return;
      // 乐观更新
      setWords((prev) => [...prev, trimmed]);
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("add_dictionary_word", { word: trimmed });
      } catch (e) {
        console.error("[useDictionary] 添加词汇失败:", e);
        setWords((prev) => prev.filter((w) => w !== trimmed));
        useUIStore.getState().showToast({ type: "warning", title: "添加词汇失败，已回滚" });
      }
    },
    [words],
  );

  const removeWord = useCallback(async (word: string) => {
    // 乐观更新
    setWords((prev) => prev.filter((w) => w !== word));
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("remove_dictionary_word", { word });
    } catch (e) {
      console.error("[useDictionary] 删除词汇失败:", e);
      setWords((prev) => [...prev, word]);
      useUIStore.getState().showToast({ type: "warning", title: "删除词汇失败，已回滚" });
    }
  }, []);

  return { words, isLoading, addWord, removeWord };
}

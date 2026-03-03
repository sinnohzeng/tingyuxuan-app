/**
 * 词典 CRUD hook。
 *
 * 封装词汇的加载、添加（乐观更新 + 失败回滚）、删除。
 * 从 DictionaryConfig.tsx 迁移核心逻辑。
 */
import { useState, useEffect, useCallback, type Dispatch, type SetStateAction } from "react";
import { useUIStore } from "../../../shared/stores/uiStore";
import { createLogger } from "../../../shared/lib/logger";

const log = createLogger("useDictionary");

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
    void loadDictionary(setWords, setIsLoading);
  }, []);

  const addWord = useCallback(
    async (word: string) => {
      const trimmed = word.trim();
      if (!trimmed || words.includes(trimmed)) return;
      // 乐观更新
      setWords((prev) => [...prev, trimmed]);
      await addDictionaryWord(trimmed, setWords);
    },
    [words],
  );

  const removeWord = useCallback(async (word: string) => {
    setWords((prev) => prev.filter((w) => w !== word));
    await removeDictionaryWord(word, setWords);
  }, []);

  return { words, isLoading, addWord, removeWord };
}

async function loadDictionary(
  setWords: (words: string[]) => void,
  setIsLoading: (loading: boolean) => void,
) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    setWords(await invoke<string[]>("get_dictionary"));
  } catch (e) {
    log.error("[useDictionary] 加载词典失败:", e);
    useUIStore.getState().showToast({ type: "error", title: "加载词典失败" });
  }
  setIsLoading(false);
}

async function addDictionaryWord(
  word: string,
  setWords: Dispatch<SetStateAction<string[]>>,
) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("add_dictionary_word", { word });
  } catch (e) {
    log.error("[useDictionary] 添加词汇失败:", e);
    setWords((prev) => prev.filter((w) => w !== word));
    useUIStore.getState().showToast({ type: "warning", title: "添加词汇失败，已回滚" });
  }
}

async function removeDictionaryWord(
  word: string,
  setWords: Dispatch<SetStateAction<string[]>>,
) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("remove_dictionary_word", { word });
  } catch (e) {
    log.error("[useDictionary] 删除词汇失败:", e);
    setWords((prev) => [...prev, word]);
    useUIStore.getState().showToast({ type: "warning", title: "删除词汇失败，已回滚" });
  }
}

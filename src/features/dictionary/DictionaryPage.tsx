/**
 * 词典管理页面 — 添加/删除词汇。
 */
import { useState, useCallback } from "react";
import { Title3, Input, Button, Text, Spinner } from "@fluentui/react-components";
import { AddRegular, BookRegular } from "@fluentui/react-icons";
import { useDictionary } from "./hooks/useDictionary";
import WordTagGrid from "./WordTagGrid";

export default function DictionaryPage() {
  const { words, isLoading, addWord, removeWord } = useDictionary();
  const [newWord, setNewWord] = useState("");

  const handleAdd = useCallback(async () => {
    const trimmed = newWord.trim();
    if (!trimmed) return;
    await addWord(trimmed);
    setNewWord("");
  }, [newWord, addWord]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spinner size="medium" label="加载词典…" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-5 p-6 max-w-4xl">
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-3">
          <Title3>个人词典</Title3>
          {words.length > 0 && (
            <Text size={200} className="text-gray-400 dark:text-gray-500 tabular-nums">
              {words.length} 词
            </Text>
          )}
        </div>
        <Text size={200} className="text-gray-500 dark:text-gray-400">
          添加专业术语、人名等词汇，帮助语音识别更准确。
        </Text>
      </div>

      <div className="flex gap-2 max-w-md">
        <Input
          className="flex-1"
          value={newWord}
          onChange={(_, data) => setNewWord(data.value)}
          placeholder="输入新词汇…"
          onKeyDown={(e) => { if (e.key === "Enter") handleAdd(); }}
        />
        <Button
          appearance="primary"
          icon={<AddRegular />}
          disabled={!newWord.trim()}
          onClick={handleAdd}
        >
          添加
        </Button>
      </div>

      {words.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-4 py-16">
          <div className="w-16 h-16 rounded-2xl bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
            <BookRegular className="text-3xl text-gray-300 dark:text-gray-600" />
          </div>
          <Text className="text-gray-400 dark:text-gray-500">
            还没有添加词汇，输入词汇开始构建你的专属词典。
          </Text>
        </div>
      ) : (
        <WordTagGrid words={words} onRemove={removeWord} />
      )}
    </div>
  );
}

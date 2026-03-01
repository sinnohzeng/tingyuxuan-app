/**
 * 词典管理页面 — 添加/删除词汇。
 */
import { useState, useCallback } from "react";
import { Title3, Input, Button, Text, Spinner } from "@fluentui/react-components";
import { AddRegular } from "@fluentui/react-icons";
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
    <div className="flex flex-col gap-4 p-6">
      <Title3>个人词典</Title3>
      <Text size={200}>
        添加专业术语、人名等词汇，帮助语音识别更准确。
      </Text>

      <div className="flex gap-2">
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

      <WordTagGrid words={words} onRemove={removeWord} />
    </div>
  );
}

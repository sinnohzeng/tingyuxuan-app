/**
 * 词汇标签网格 — flex-wrap 布局，每个 tag 可关闭。
 */
import { Tag, Text } from "@fluentui/react-components";
import { BookRegular } from "@fluentui/react-icons";

interface WordTagGridProps {
  words: string[];
  onRemove: (word: string) => void;
}

export default function WordTagGrid({ words, onRemove }: WordTagGridProps) {
  if (words.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-12">
        <BookRegular className="text-4xl text-gray-300" />
        <Text>词典为空，添加常用词汇以提高识别准确率。</Text>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-2">
      {words.map((word) => (
        <Tag
          key={word}
          dismissible
          dismissIcon={{ "aria-label": "删除" }}
          value={word}
          onClick={() => onRemove(word)}
        >
          {word}
        </Tag>
      ))}
    </div>
  );
}

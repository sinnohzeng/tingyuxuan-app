#!/usr/bin/env node
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";

const THRESHOLDS = {
  fileLines: 800,
  functionLines: 30,
  branches: 3,
  nesting: 3,
};

const ROOT = process.cwd();

const FILE_RULES = [
  {
    ext: [".ts", ".tsx"],
    include: /^src\//,
    exclude: [
      /\.test\.(ts|tsx)$/,
      /\/test-setup\.ts$/,
      /\/test-utils\.tsx$/,
      /^src\/components\/.*\.test\.tsx$/,
    ],
    language: "TypeScript",
    patterns: [
      /^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_]\w*)/,
      /^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_]\w*)\s*=\s*(?:async\s*)?(?:\([^;]*\)|[A-Za-z_]\w*)\s*=>\s*\{/,
    ],
    branchRegex: /\b(if|else\s+if|switch)\b/g,
    controlRegex:
      /\b(else\s+if|if|else|switch|for|while|try|catch)\b/g,
  },
  {
    ext: [".rs"],
    include: /^(src-tauri\/src\/|crates\/[^/]+\/src\/)/,
    exclude: [/\/tests?\//],
    language: "Rust",
    patterns: [/^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_]\w*)/],
    branchRegex: /\b(if|else\s+if|match)\b/g,
    controlRegex: /\b(else\s+if|if|else|match|for|while|loop)\b/g,
  },
  {
    ext: [".kt", ".kts"],
    include: /^android\/app\/src\/main\//,
    exclude: [],
    language: "Kotlin",
    patterns: [
      /^\s*(?:(?:private|public|internal|protected|suspend|override|inline|tailrec|operator|open|final|abstract|data|sealed)\s+)*fun\s+([A-Za-z_]\w*)/,
    ],
    branchRegex: /\b(if|else\s+if|when)\b/g,
    controlRegex: /\b(else\s+if|if|else|when|for|while|try|catch)\b/g,
  },
];

function getGitFiles() {
  const raw = execSync("git ls-files", { encoding: "utf-8" });
  return raw
    .split(/\r?\n/)
    .map((f) => f.trim())
    .filter(Boolean);
}

function pickRule(file) {
  const ext = path.extname(file).toLowerCase();
  return FILE_RULES.find((rule) => rule.ext.includes(ext));
}

function isIncluded(file, rule) {
  if (!rule.include.test(file)) {
    return false;
  }
  return !rule.exclude.some((re) => re.test(file));
}

function countBranches(line, branchRegex) {
  const matches = line.match(branchRegex);
  return matches ? matches.length : 0;
}

function stringDelimitersFor(language) {
  if (language === "Rust") {
    return ['"'];
  }
  return ['"', "'", "`"];
}

function sanitizeLines(lines, language) {
  const delimiters = new Set(stringDelimitersFor(language));
  const sanitized = [];
  let inBlockComment = false;
  let stringDelimiter = null;

  for (const line of lines) {
    let result = "";
    for (let i = 0; i < line.length; i += 1) {
      const current = line[i];
      const next = i + 1 < line.length ? line[i + 1] : "";

      if (inBlockComment) {
        if (current === "*" && next === "/") {
          inBlockComment = false;
          i += 1;
        }
        continue;
      }

      if (stringDelimiter) {
        if (current === "\\" && i + 1 < line.length) {
          i += 1;
          continue;
        }
        if (current === stringDelimiter) {
          stringDelimiter = null;
        }
        continue;
      }

      if (current === "/" && next === "/") {
        break;
      }
      if (current === "/" && next === "*") {
        inBlockComment = true;
        i += 1;
        continue;
      }
      if (delimiters.has(current)) {
        stringDelimiter = current;
        continue;
      }

      result += current;
    }
    sanitized.push(result);
  }

  return sanitized;
}

function isMeaningfulLine(line) {
  const trimmed = line.trim();
  if (!trimmed) {
    return false;
  }
  return !/^[{}()[\];,]+$/.test(trimmed);
}

function findFunctions(lines, rule) {
  const functions = [];
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    for (const pattern of rule.patterns) {
      const match = line.match(pattern);
      if (match) {
        functions.push({
          name: match[1],
          line: i + 1,
          startIndex: i,
          composable: rule.language === "Kotlin" ? hasComposableAnnotation(lines, i) : false,
        });
        break;
      }
    }
  }
  return functions;
}

function hasComposableAnnotation(lines, fnStartIndex) {
  for (let i = fnStartIndex - 1; i >= 0 && i >= fnStartIndex - 4; i -= 1) {
    const line = lines[i].trim();
    if (!line) {
      continue;
    }
    return line.startsWith("@Composable");
  }
  return false;
}

function detectRustTestRanges(lines) {
  const ranges = [];
  let i = 0;
  while (i < lines.length) {
    if (!/^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/.test(lines[i])) {
      i += 1;
      continue;
    }

    let j = i + 1;
    while (j < lines.length && /^\s*$/.test(lines[j])) {
      j += 1;
    }
    if (j >= lines.length || !/^\s*mod\s+\w+\s*\{/.test(lines[j])) {
      i = j;
      continue;
    }

    let depth = 0;
    let started = false;
    let end = j;
    for (let k = j; k < lines.length; k += 1) {
      for (const ch of lines[k]) {
        if (ch === "{") {
          depth += 1;
          started = true;
        } else if (ch === "}") {
          depth -= 1;
          if (started && depth === 0) {
            end = k;
            break;
          }
        }
      }
      if (started && depth === 0) {
        break;
      }
    }

    ranges.push([i, end]);
    i = end + 1;
  }
  return ranges;
}

function inRanges(index, ranges) {
  return ranges.some(([start, end]) => index >= start && index <= end);
}

function findBodyStart(lines, fnStart) {
  const maxLookahead = Math.min(lines.length, fnStart + 12);
  for (let i = fnStart; i < maxLookahead; i += 1) {
    const line = lines[i];
    if (line.includes("{")) {
      return i;
    }
    if (line.includes(";")) {
      return -1;
    }
  }
  return -1;
}

function analyzeFunction(lines, fn, rule) {
  const bodyStart = findBodyStart(lines, fn.startIndex);
  if (bodyStart < 0) {
    return null;
  }

  let depth = 0;
  let endIndex = lines.length - 1;
  let branches = 0;
  let controlDepth = 0;
  let maxControlDepth = 0;
  const controlStack = [];

  for (let i = bodyStart; i < lines.length; i += 1) {
    const line = lines[i];
    branches += countBranches(line, rule.branchRegex);
    let controlOpenings = countBranches(line, rule.controlRegex);

    for (const ch of line) {
      if (ch === "{") {
        depth += 1;
        const isControl = controlOpenings > 0;
        if (isControl) {
          controlOpenings -= 1;
          controlDepth += 1;
          if (controlDepth > maxControlDepth) {
            maxControlDepth = controlDepth;
          }
        }
        controlStack.push(isControl);
      } else if (ch === "}") {
        const wasControl = controlStack.pop();
        if (wasControl) {
          controlDepth = Math.max(0, controlDepth - 1);
        }
        depth -= 1;
        if (depth === 0) {
          endIndex = i;
          break;
        }
      }
    }
    if (depth === 0 && i >= bodyStart) {
      break;
    }
  }

  let length = 0;
  for (let i = fn.startIndex; i <= endIndex; i += 1) {
    if (isMeaningfulLine(lines[i])) {
      length += 1;
    }
  }

  return {
    length,
    branches,
    nesting: maxControlDepth,
  };
}

function analyzeFile(file, rule, results) {
  const text = readFileSync(path.join(ROOT, file), "utf-8");
  const lines = text.split(/\r?\n/);
  const sanitizedLines = sanitizeLines(lines, rule.language);
  const rustTestRanges =
    rule.language === "Rust" ? detectRustTestRanges(sanitizedLines) : [];
  const effectiveLineCount =
    rule.language === "Rust"
      ? sanitizedLines.filter(
          (line, idx) => !inRanges(idx, rustTestRanges) && isMeaningfulLine(line),
        ).length
      : sanitizedLines.filter((line) => isMeaningfulLine(line)).length;

  if (effectiveLineCount > THRESHOLDS.fileLines) {
    results.fileViolations.push({
      file,
      lines: effectiveLineCount,
    });
  }

  const functions = findFunctions(sanitizedLines, rule);
  for (const fn of functions) {
    if (
      rule.language === "Rust" &&
      (fn.name.startsWith("test_") || inRanges(fn.startIndex, rustTestRanges))
    ) {
      continue;
    }
    if (rule.language === "Kotlin" && fn.composable) {
      continue;
    }
    const metrics = analyzeFunction(sanitizedLines, fn, rule);
    if (!metrics) {
      continue;
    }
    if (
      metrics.length > THRESHOLDS.functionLines ||
      metrics.branches > THRESHOLDS.branches ||
      metrics.nesting > THRESHOLDS.nesting
    ) {
      results.functionViolations.push({
        file,
        language: rule.language,
        name: fn.name,
        line: fn.line,
        length: metrics.length,
        branches: metrics.branches,
        nesting: metrics.nesting,
      });
    }
  }
}

function printResults(results) {
  const totalViolations =
    results.fileViolations.length + results.functionViolations.length;
  console.log("=== Redline Quality Report ===");
  console.log(
    `Thresholds: file<=${THRESHOLDS.fileLines}, function<=${THRESHOLDS.functionLines}, branches<=${THRESHOLDS.branches}, nesting<=${THRESHOLDS.nesting}`,
  );
  console.log(`Scanned files: ${results.scannedFiles}`);
  console.log(`Total violations: ${totalViolations}`);

  if (results.fileViolations.length > 0) {
    console.log("\n[File line violations]");
    for (const v of results.fileViolations) {
      console.log(`- ${v.file}: ${v.lines} lines`);
    }
  }

  if (results.functionViolations.length > 0) {
    console.log("\n[Function violations]");
    const sorted = [...results.functionViolations].sort((a, b) => {
      const scoreA =
        Math.max(0, a.length - THRESHOLDS.functionLines) +
        Math.max(0, a.branches - THRESHOLDS.branches) * 8 +
        Math.max(0, a.nesting - THRESHOLDS.nesting) * 12;
      const scoreB =
        Math.max(0, b.length - THRESHOLDS.functionLines) +
        Math.max(0, b.branches - THRESHOLDS.branches) * 8 +
        Math.max(0, b.nesting - THRESHOLDS.nesting) * 12;
      return scoreB - scoreA;
    });

    for (const v of sorted) {
      console.log(
        `- ${v.file}:${v.line} ${v.name} [${v.language}] lines=${v.length}, branches=${v.branches}, nesting=${v.nesting}`,
      );
    }
  }
}

function main() {
  const gitFiles = getGitFiles();
  const results = {
    scannedFiles: 0,
    fileViolations: [],
    functionViolations: [],
  };

  for (const file of gitFiles) {
    const rule = pickRule(file);
    if (!rule || !isIncluded(file, rule)) {
      continue;
    }
    results.scannedFiles += 1;
    analyzeFile(file, rule, results);
  }

  printResults(results);
  const failed =
    results.fileViolations.length > 0 || results.functionViolations.length > 0;
  if (failed) {
    process.exit(1);
  }
}

main();

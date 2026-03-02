#!/usr/bin/env node
/**
 * SLS MCP Server — 封装阿里云 SLS GetLogs API，供 Claude Code 查询 telemetry 数据。
 *
 * 使用 HMAC-SHA1 签名的 GetLogs API（只有 MCP server 端需要签名，客户端用 Web Tracking 无需）。
 *
 * 环境变量：
 *   SLS_ENDPOINT           — SLS 区域 endpoint（如 cn-hangzhou.log.aliyuncs.com）
 *   SLS_PROJECT            — SLS Project 名称
 *   SLS_LOGSTORE           — SLS Logstore 名称
 *   SLS_ACCESS_KEY_ID      — 阿里云 AccessKey ID
 *   SLS_ACCESS_KEY_SECRET  — 阿里云 AccessKey Secret
 */
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import crypto from "node:crypto";

const ENDPOINT = process.env.SLS_ENDPOINT || "";
const PROJECT = process.env.SLS_PROJECT || "";
const LOGSTORE = process.env.SLS_LOGSTORE || "";
const AK_ID = process.env.SLS_ACCESS_KEY_ID || "";
const AK_SECRET = process.env.SLS_ACCESS_KEY_SECRET || "";

function hmacSha1(key, data) {
  return crypto.createHmac("sha1", key).update(data).digest("base64");
}

function rfcDate() {
  return new Date().toUTCString();
}

async function slsGetLogs(query, from, to, limit = 100) {
  const host = `${PROJECT}.${ENDPOINT}`;
  const path = `/logstores/${LOGSTORE}`;
  const date = rfcDate();
  const params = new URLSearchParams({
    type: "log",
    query,
    from: String(from),
    to: String(to),
    line: String(limit),
  });

  const resource = `${path}?${params.toString()}`;
  const stringToSign = `GET\n\n\n${date}\nx-log-apiversion:0.6.0\nx-log-signaturemethod:hmac-sha1\n${resource}`;
  const signature = hmacSha1(AK_SECRET, stringToSign);

  const url = `https://${host}${path}?${params.toString()}`;
  const resp = await fetch(url, {
    headers: {
      Host: host,
      Date: date,
      "x-log-apiversion": "0.6.0",
      "x-log-signaturemethod": "hmac-sha1",
      Authorization: `LOG ${AK_ID}:${signature}`,
    },
  });

  if (!resp.ok) {
    throw new Error(`SLS API ${resp.status}: ${await resp.text()}`);
  }
  return resp.json();
}

function timeRange(period) {
  const now = Math.floor(Date.now() / 1000);
  switch (period) {
    case "1h":
      return [now - 3600, now];
    case "24h":
      return [now - 86400, now];
    case "7d":
      return [now - 604800, now];
    default:
      return [now - 86400, now];
  }
}

const server = new Server(
  { name: "sls-mcp", version: "0.1.0" },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "sls_query",
      description: "执行 SLS 查询（支持 SQL 语法，如 SELECT ... WHERE event_type = ...）",
      inputSchema: {
        type: "object",
        properties: {
          query: { type: "string", description: "SLS 查询语句" },
          period: { type: "string", description: "时间范围：1h / 24h / 7d（默认 24h）" },
          limit: { type: "number", description: "最大返回条数（默认 100）" },
        },
        required: ["query"],
      },
    },
    {
      name: "sls_get_session",
      description: "按 session_id 查询完整事件链",
      inputSchema: {
        type: "object",
        properties: {
          session_id: { type: "string", description: "Session ID" },
        },
        required: ["session_id"],
      },
    },
    {
      name: "sls_error_stats",
      description: "近 24h/7d 错误统计（按 error_code 分组）",
      inputSchema: {
        type: "object",
        properties: {
          period: { type: "string", description: "时间范围：24h / 7d（默认 24h）" },
        },
      },
    },
    {
      name: "sls_performance",
      description: "P50/P95 性能指标（LLM 延迟、编码耗时等）",
      inputSchema: {
        type: "object",
        properties: {
          period: { type: "string", description: "时间范围：24h / 7d（默认 24h）" },
        },
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case "sls_query": {
        const [from, to] = timeRange(args?.period || "24h");
        const result = await slsGetLogs(
          args.query,
          from,
          to,
          args?.limit || 100,
        );
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }
      case "sls_get_session": {
        const [from, to] = timeRange("7d");
        const result = await slsGetLogs(
          `session_id: "${args.session_id}"`,
          from,
          to,
          50,
        );
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }
      case "sls_error_stats": {
        const [from, to] = timeRange(args?.period || "24h");
        const result = await slsGetLogs(
          `event_type: "session_failed" | SELECT error_code, COUNT(*) as count GROUP BY error_code ORDER BY count DESC`,
          from,
          to,
          50,
        );
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }
      case "sls_performance": {
        const [from, to] = timeRange(args?.period || "24h");
        const result = await slsGetLogs(
          `event_type: "session_completed" | SELECT approx_percentile(llm_total_ms, 0.5) as p50_llm, approx_percentile(llm_total_ms, 0.95) as p95_llm, approx_percentile(recording_ms, 0.5) as p50_recording, AVG(result_chars) as avg_chars, COUNT(*) as total`,
          from,
          to,
          10,
        );
        return {
          content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        };
      }
      default:
        return {
          content: [{ type: "text", text: `Unknown tool: ${name}` }],
          isError: true,
        };
    }
  } catch (error) {
    return {
      content: [{ type: "text", text: `Error: ${error.message}` }],
      isError: true,
    };
  }
});

const transport = new StdioServerTransport();
await server.connect(transport);

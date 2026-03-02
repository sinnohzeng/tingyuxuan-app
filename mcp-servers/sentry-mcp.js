#!/usr/bin/env node
/**
 * Sentry MCP Server — 封装 Sentry REST API，供 Claude Code 查询错误/崩溃信息。
 *
 * 环境变量：
 *   SENTRY_URL          — Sentry 实例地址（默认 https://sentry.io）
 *   SENTRY_AUTH_TOKEN    — Sentry API Token
 *   SENTRY_ORG           — 组织 slug
 *   SENTRY_PROJECT       — 项目 slug
 */
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

const SENTRY_URL = process.env.SENTRY_URL || "https://sentry.io";
const AUTH_TOKEN = process.env.SENTRY_AUTH_TOKEN || "";
const ORG = process.env.SENTRY_ORG || "";
const PROJECT = process.env.SENTRY_PROJECT || "";

async function sentryApi(path, params = {}) {
  const url = new URL(`/api/0/${path}`, SENTRY_URL);
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null) url.searchParams.set(k, String(v));
  }
  const resp = await fetch(url.toString(), {
    headers: { Authorization: `Bearer ${AUTH_TOKEN}` },
  });
  if (!resp.ok) {
    throw new Error(`Sentry API ${resp.status}: ${await resp.text()}`);
  }
  return resp.json();
}

const server = new Server(
  { name: "sentry-mcp", version: "0.1.0" },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "sentry_list_issues",
      description: "列出 Sentry 近期错误/问题（默认最近 24h 未解决的）",
      inputSchema: {
        type: "object",
        properties: {
          query: { type: "string", description: "搜索查询（Sentry 查询语法）" },
          limit: { type: "number", description: "最大返回数（默认 10）" },
        },
      },
    },
    {
      name: "sentry_get_issue",
      description: "获取 Sentry 问题详情（含堆栈、标签、首次/末次出现）",
      inputSchema: {
        type: "object",
        properties: {
          issue_id: { type: "string", description: "Issue ID" },
        },
        required: ["issue_id"],
      },
    },
    {
      name: "sentry_get_latest_event",
      description: "获取某个 Issue 的最新事件（含 breadcrumbs、堆栈、设备信息）",
      inputSchema: {
        type: "object",
        properties: {
          issue_id: { type: "string", description: "Issue ID" },
        },
        required: ["issue_id"],
      },
    },
    {
      name: "sentry_search",
      description: "按关键词搜索 Sentry 问题",
      inputSchema: {
        type: "object",
        properties: {
          keyword: { type: "string", description: "搜索关键词" },
          limit: { type: "number", description: "最大返回数（默认 10）" },
        },
        required: ["keyword"],
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case "sentry_list_issues": {
        const query = args?.query || "is:unresolved";
        const limit = args?.limit || 10;
        const issues = await sentryApi(
          `projects/${ORG}/${PROJECT}/issues/`,
          { query, limit },
        );
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(
                issues.map((i) => ({
                  id: i.id,
                  title: i.title,
                  culprit: i.culprit,
                  count: i.count,
                  firstSeen: i.firstSeen,
                  lastSeen: i.lastSeen,
                  level: i.level,
                  status: i.status,
                })),
                null,
                2,
              ),
            },
          ],
        };
      }
      case "sentry_get_issue": {
        const issue = await sentryApi(`issues/${args.issue_id}/`);
        return {
          content: [{ type: "text", text: JSON.stringify(issue, null, 2) }],
        };
      }
      case "sentry_get_latest_event": {
        const event = await sentryApi(
          `issues/${args.issue_id}/events/latest/`,
        );
        return {
          content: [{ type: "text", text: JSON.stringify(event, null, 2) }],
        };
      }
      case "sentry_search": {
        const limit = args?.limit || 10;
        const issues = await sentryApi(
          `projects/${ORG}/${PROJECT}/issues/`,
          { query: args.keyword, limit },
        );
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(
                issues.map((i) => ({
                  id: i.id,
                  title: i.title,
                  culprit: i.culprit,
                  count: i.count,
                  lastSeen: i.lastSeen,
                  level: i.level,
                })),
                null,
                2,
              ),
            },
          ],
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

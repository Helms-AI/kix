import { useState, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import {
  Plug2,
  Terminal,
  Monitor,
  Code2,
  Search,
  Brain,
  Database,
  Copy,
  Check,
  AlertCircle,
  Zap,
  ChevronRight,
  ExternalLink,
  Sparkles,
  Settings2,
  Bot,
  Webhook,
  FileText,
  Workflow,
  MessageSquare,
  Bell,
  Globe,
  Network,
  CheckCircle2,
  Server,
  Layers,
} from 'lucide-react';
import clsx from 'clsx';

// ============================================
// DATA STRUCTURES
// ============================================

// Transport types
type TransportType = 'http' | 'stdio' | 'both';

// Transport support for each client
interface TransportSupport {
  supported: boolean;
  recommended?: boolean;
  note?: string;
}

interface ClientTransports {
  http?: TransportSupport;
  stdio?: TransportSupport;
}

// LucideIcon type alias
type LucideIcon = React.ComponentType<{ className?: string }>;

// Transport options data
const transportOptions = [
  {
    id: 'unified' as const,
    name: 'Unified',
    subtitle: 'HTTP + stdio',
    icon: Layers,
    color: 'emerald',
    description: 'Run both transports simultaneously',
    command: './run.sh',
    altCommand: 'kix run --stdio',
    features: ['Web dashboard access', 'IDE integration', 'Full API + MCP'],
    recommended: true,
    ports: ['REST API: 3001', 'MCP HTTP: 3002', 'stdio: enabled'],
  },
  {
    id: 'http' as const,
    name: 'HTTP Only',
    subtitle: 'Network transport',
    icon: Globe,
    color: 'cyan',
    description: 'For web clients and remote connections',
    command: 'kix run',
    altCommand: 'kix run --api-port 3001 --mcp-port 3002',
    features: ['Web browser clients', 'Remote connections', 'CORS support via proxy'],
    recommended: false,
    ports: ['REST API: 3001', 'MCP HTTP: 3002'],
  },
  {
    id: 'stdio' as const,
    name: 'stdio Only',
    subtitle: 'Direct process',
    icon: Terminal,
    color: 'violet',
    description: 'For desktop apps and CLI tools',
    command: 'kix serve',
    altCommand: null,
    features: ['No network overhead', 'Direct IPC', 'Claude Desktop'],
    recommended: false,
    ports: ['stdio: stdin/stdout'],
  },
];

// Client configuration data with multi-transport support
interface ClientConfig {
  id: string;
  name: string;
  icon: LucideIcon;
  description: string;
  transports: ClientTransports;
  cliCommands?: {
    http?: string;
    stdio?: string;
  };
  configPath: string;
  configPathAlt?: string;
  configs: {
    http?: string;
    stdio?: string;
  };
  note?: string;
}

const clients: ClientConfig[] = [
  {
    id: 'claude-desktop',
    name: 'Claude Desktop',
    icon: Monitor,
    description: 'Desktop application',
    transports: {
      stdio: { supported: true, recommended: true },
    },
    cliCommands: {
      stdio: 'claude mcp add kix -- kix run --stdio',
    },
    configPath: '~/Library/Application Support/Claude/claude_desktop_config.json',
    configPathAlt: '%APPDATA%\\Claude\\claude_desktop_config.json (Windows)',
    configs: {
      stdio: `{
  "mcpServers": {
    "kix": {
      "command": "kix",
      "args": ["run", "--stdio"]
    }
  }
}`,
    },
    note: 'Requires stdio transport - direct process communication',
  },
  {
    id: 'claude-code',
    name: 'Claude Code',
    icon: Terminal,
    description: 'Anthropic CLI tool',
    transports: {
      http: { supported: true, recommended: true, note: 'Best for remote connections' },
      stdio: { supported: true, note: 'Best for direct binary integration' },
    },
    cliCommands: {
      http: 'claude mcp add kix --transport http http://localhost:3000/mcp',
      stdio: 'claude mcp add kix -- kix run --stdio',
    },
    configPath: '~/.claude/settings.json',
    configs: {
      http: `{
  "mcpServers": {
    "kix": {
      "type": "http",
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
      stdio: `{
  "mcpServers": {
    "kix": {
      "command": "kix",
      "args": ["run", "--stdio"]
    }
  }
}`,
    },
  },
  {
    id: 'cursor',
    name: 'Cursor',
    icon: Code2,
    description: 'AI-first code editor',
    transports: {
      http: { supported: true, recommended: true },
      stdio: { supported: true, note: 'Via command configuration' },
    },
    cliCommands: {
      http: 'cursor --add-mcp kix http://localhost:3000/mcp',
    },
    configPath: '.cursor/mcp.json',
    configs: {
      http: `{
  "mcpServers": {
    "kix": {
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
      stdio: `{
  "mcpServers": {
    "kix": {
      "command": "kix",
      "args": ["run", "--stdio"]
    }
  }
}`,
    },
  },
  {
    id: 'windsurf',
    name: 'Windsurf',
    icon: Sparkles,
    description: 'Codeium IDE',
    transports: {
      http: { supported: true, recommended: true },
    },
    cliCommands: {
      http: 'windsurf --add-mcp kix http://localhost:3000/mcp',
    },
    configPath: '~/.codeium/windsurf/mcp_config.json',
    configs: {
      http: `{
  "mcpServers": {
    "kix": {
      "serverUrl": "http://localhost:3000/mcp"
    }
  }
}`,
    },
  },
  {
    id: 'vscode',
    name: 'VS Code',
    icon: Code2,
    description: 'With MCP extension',
    transports: {
      http: { supported: true, recommended: true },
      stdio: { supported: true, note: 'Extension-dependent' },
    },
    configPath: '.vscode/settings.json',
    configs: {
      http: `{
  "mcp.servers": {
    "kix": {
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
      stdio: `{
  "mcp.servers": {
    "kix": {
      "command": "kix",
      "args": ["run", "--stdio"]
    }
  }
}`,
    },
    note: 'Requires MCP extension to be installed',
  },
];

// Helper to get primary transport for a client
function getPrimaryTransport(transports: ClientTransports): 'http' | 'stdio' {
  if (transports.http?.recommended) return 'http';
  if (transports.stdio?.recommended) return 'stdio';
  if (transports.http?.supported) return 'http';
  return 'stdio';
}

// Helper to check if client supports multiple transports
function hasMultipleTransports(transports: ClientTransports): boolean {
  return !!(transports.http?.supported && transports.stdio?.supported);
}

// Tool categories for Installation tab
const toolCategories = [
  {
    name: 'Retrieval',
    icon: Search,
    color: 'cyan',
    tools: [
      { name: 'search', description: 'Unified semantic + keyword search with hybrid/vector/text modes and filtering' },
      { name: 'get_context', description: 'Retrieve full page content for RAG synthesis via page_id or chunk_id' },
      { name: 'get_document', description: 'Get document metadata with optional chunk details' },
    ],
  },
  {
    name: 'Indexing',
    icon: Database,
    color: 'amber',
    tools: [
      { name: 'index', description: 'Index a single document from text, file path, or URL' },
      { name: 'index_async', description: 'Start async crawl/batch indexing with job_id for progress tracking' },
      { name: 'job_status', description: 'Check progress of async indexing jobs' },
      { name: 'delete', description: 'Remove documents by ID, multiple IDs, or filter (tag/domain)' },
    ],
  },
  {
    name: 'Status',
    icon: Brain,
    color: 'violet',
    tools: [
      { name: 'status', description: 'Get index health and statistics with optional detailed breakdown' },
    ],
  },
];

// Troubleshooting items - expanded with transport-specific issues
const troubleshooting = [
  {
    issue: 'HTTP connection refused',
    solution: 'Ensure KIX server is running: ./run.sh or kix run --api-port 3001 --mcp-port 3002',
    transport: 'http',
  },
  {
    issue: 'stdio transport not working',
    solution: 'Verify kix binary is in PATH (which kix) and server started with --stdio flag',
    transport: 'stdio',
  },
  {
    issue: 'CORS errors in browser',
    solution: 'Use the proxied endpoint at port 3000 instead of direct connection to port 3002',
    transport: 'http',
  },
  {
    issue: 'Claude Desktop not connecting',
    solution: 'Check config uses ["run", "--stdio"] args. Ensure kix binary is accessible.',
    transport: 'stdio',
  },
  {
    issue: 'Tools not appearing',
    solution: 'Restart your AI client after config changes. Verify JSON syntax is valid.',
    transport: 'both',
  },
  {
    issue: 'Slow first response',
    solution: 'Normal - embedding models initialize on first request. Subsequent calls are fast.',
    transport: 'both',
  },
];

// Agent tools for sub-tabs (subset of clients without Desktop for HTTP-based agents)
const agentTools = clients.filter(c => c.transports.http?.supported);

// Agent content templates per tool
const agentContent: Record<string, {
  claudeMd: { title: string; description: string; content: string };
  workflow: { title: string; description: string; content: string };
  systemPrompt: { title: string; description: string; content: string };
}> = {
  'claude-code': {
    claudeMd: {
      title: 'CLAUDE.md Template',
      description: 'Add this to your project\'s CLAUDE.md file for KIX integration instructions',
      content: `# KIX RAG Integration

## Knowledge Search
When you need to find documentation or code examples:
1. Use \`mcp__kix__search\` with natural language queries
   - Supports modes: \`hybrid\` (default), \`vector\`, \`text\`
   - Filter by: entry_type, chunk_type, tag, source_domain
2. For full page context (RAG synthesis), use \`mcp__kix__get_context\` with the page_id from search results
3. Use \`mcp__kix__get_document\` with include_chunks=true for detailed document info

## Indexing New Content
To add documentation to the knowledge base:
1. Use \`mcp__kix__index\` for single documents (text, file path, or URL)
2. Use \`mcp__kix__index_async\` for crawling websites with depth/max_pages settings
3. Check progress with \`mcp__kix__job_status\` using the returned job_id

## Managing Content
- Use \`mcp__kix__delete\` with id, ids, or filter (tag/source_domain) to remove documents
- Use \`mcp__kix__status\` with detailed=true to see breakdown by type and domain

## Best Practices
- Search before indexing to avoid duplicates
- Use descriptive tags for organization and filtering
- Prefer hybrid search mode for best results
- Always use get_context for RAG synthesis after search`,
    },
    workflow: {
      title: 'Workflow Configuration',
      description: 'Agent workflow definition for automated RAG lookups',
      content: `{
  "name": "kix-rag-workflow",
  "description": "Automated RAG workflow for documentation lookup",
  "triggers": ["search", "research", "documentation", "how to", "what is"],
  "steps": [
    {
      "action": "mcp__kix__search",
      "params": {
        "query": "\${user_query}",
        "mode": "hybrid",
        "limit": 10
      }
    },
    {
      "action": "mcp__kix__get_context",
      "condition": "results.length > 0",
      "params": {
        "page_id": "\${results[0].page_id}"
      }
    }
  ]
}`,
    },
    systemPrompt: {
      title: 'System Prompt',
      description: 'Pre-built prompt template for RAG-enabled assistant behavior',
      content: `## KIX RAG Search Assistant

You have access to KIX, a knowledge indexing system with semantic search capabilities.

### Workflow
1. **Search First**: Always query the knowledge base before answering documentation questions
   - Use \`search\` with natural language queries
   - Set mode to "hybrid" for best results

2. **Get Full Context**: For relevant results, retrieve the full page
   - Use \`get_context\` with the page_id from search results
   - This provides complete context for accurate answers

3. **Cite Sources**: Reference document titles and page IDs in your answers
   - Format: "According to [Document Title] (page_id: xxx)..."

4. **Synthesize**: Combine multiple sources into coherent, accurate answers

5. **Index Gaps**: When you can't find relevant information, suggest:
   - "I couldn't find documentation on X. Would you like me to index a URL?"`,
    },
  },
  'cursor': {
    claudeMd: {
      title: 'Cursor Rules Template',
      description: 'Add to .cursorrules file for KIX integration in Cursor IDE',
      content: `# KIX Knowledge Integration for Cursor

## Available MCP Tools
You have access to KIX through the following MCP tools:
- \`search\` - Search the knowledge base with natural language
- \`get_context\` - Get full page content for detailed answers
- \`index\` - Add new documents to the knowledge base
- \`status\` - Check index health and statistics

## Usage Guidelines
1. When the user asks about documented topics, search KIX first
2. Use hybrid search mode for best results
3. Always retrieve full context before providing detailed answers
4. Cite sources with document titles when available

## Example Queries
- "Search for authentication patterns"
- "Find React component best practices"
- "Look up API documentation for user endpoints"`,
    },
    workflow: {
      title: 'Cursor Workflow',
      description: 'Workflow configuration for Cursor AI composer',
      content: `{
  "name": "cursor-kix-workflow",
  "description": "KIX integration for Cursor composer",
  "context_triggers": ["@docs", "@knowledge", "@search"],
  "auto_search": {
    "enabled": true,
    "keywords": ["documentation", "how to", "example", "usage"]
  },
  "search_config": {
    "mode": "hybrid",
    "limit": 5,
    "include_context": true
  }
}`,
    },
    systemPrompt: {
      title: 'Cursor System Prompt',
      description: 'System prompt optimized for Cursor AI interactions',
      content: `You are a code assistant with access to KIX knowledge base.

When working with code:
1. Search KIX for relevant documentation before suggesting implementations
2. Reference found documentation in your explanations
3. Suggest indexing missing documentation when gaps are found

Search patterns:
- For API questions: search for endpoint names or routes
- For component questions: search for component patterns
- For configuration: search for config files or setup guides

Always prioritize accuracy over speed - verify against documentation.`,
    },
  },
  'windsurf': {
    claudeMd: {
      title: 'Windsurf Config Template',
      description: 'Configuration for KIX integration in Windsurf IDE',
      content: `# KIX Integration for Windsurf

## MCP Server Configuration
KIX provides semantic search over your indexed documentation.

## Available Tools
- \`mcp__kix__search\` - Hybrid semantic + keyword search
- \`mcp__kix__get_context\` - Full page retrieval for RAG
- \`mcp__kix__index\` - Add documents to knowledge base
- \`mcp__kix__index_async\` - Crawl and index websites
- \`mcp__kix__status\` - Check system health

## Cascade Integration
When Cascade is active, automatically search KIX for:
- Documentation references
- Code examples
- API specifications
- Best practices

## Tips
- Use tags to organize indexed content
- Search with natural language queries
- Retrieve full context for comprehensive answers`,
    },
    workflow: {
      title: 'Cascade Workflow',
      description: 'Workflow for Windsurf Cascade AI',
      content: `{
  "name": "windsurf-cascade-kix",
  "description": "KIX integration for Cascade flows",
  "cascade_triggers": {
    "documentation_lookup": true,
    "code_explanation": true,
    "api_reference": true
  },
  "search_settings": {
    "mode": "hybrid",
    "auto_context": true,
    "max_results": 8
  }
}`,
    },
    systemPrompt: {
      title: 'Cascade System Prompt',
      description: 'System prompt for Windsurf Cascade AI',
      content: `You are Cascade with KIX knowledge integration.

## Knowledge Access
Use KIX MCP tools to search and retrieve documentation:
- Search first, then provide informed answers
- Get full context for detailed explanations
- Suggest indexing when documentation is missing

## Workflow
1. Identify if query needs documentation lookup
2. Search KIX with relevant terms
3. Retrieve context for top results
4. Synthesize answer from found content
5. Cite sources appropriately`,
    },
  },
  'vscode': {
    claudeMd: {
      title: 'VS Code MCP Config',
      description: 'Settings for VS Code MCP extension with KIX',
      content: `# KIX Integration for VS Code

## Prerequisites
- Install the MCP extension for VS Code
- Ensure KIX server is running on localhost:3000/mcp

## Configuration
Add to your .vscode/settings.json:
\`\`\`json
{
  "mcp.servers": {
    "kix": {
      "url": "http://localhost:3000/mcp"
    }
  }
}
\`\`\`

## Available Tools
Once configured, you'll have access to:
- search - Semantic search across indexed content
- get_context - Retrieve full pages for detailed info
- index - Add new documents to the knowledge base
- status - Check index health

## Usage
Invoke MCP tools through the VS Code command palette or inline completions.`,
    },
    workflow: {
      title: 'VS Code Tasks',
      description: 'Task configuration for automated KIX operations',
      content: `{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "KIX: Search Documentation",
      "type": "shell",
      "command": "kix",
      "args": ["search", "\${input:searchQuery}", "--limit", "10"],
      "problemMatcher": []
    },
    {
      "label": "KIX: Index Current File",
      "type": "shell",
      "command": "kix",
      "args": ["index", "--file", "\${file}"],
      "problemMatcher": []
    },
    {
      "label": "KIX: Check Status",
      "type": "shell",
      "command": "kix",
      "args": ["stats"],
      "problemMatcher": []
    }
  ],
  "inputs": [
    {
      "id": "searchQuery",
      "description": "Search query",
      "type": "promptString"
    }
  ]
}`,
    },
    systemPrompt: {
      title: 'Copilot System Prompt',
      description: 'System prompt for GitHub Copilot with KIX',
      content: `You are a VS Code assistant with access to KIX knowledge base.

When the user asks about documented topics:
1. Use KIX search to find relevant documentation
2. Retrieve full context for accurate answers
3. Provide code examples from indexed sources

For code suggestions:
- Search for related patterns and examples
- Reference documentation for API usage
- Suggest best practices from indexed content

Always verify suggestions against documentation when available.`,
    },
  },
};

// Hooks content templates per tool
const hooksContent: Record<string, {
  claudeHooks: { title: string; description: string; content: string };
  mcpEvents: { title: string; description: string; content: string };
}> = {
  'claude-code': {
    claudeHooks: {
      title: 'Claude Code Hooks',
      description: 'Pre/post command hooks for Claude Code CLI (.claude/hooks.json)',
      content: `{
  "hooks": [
    {
      "matcher": {
        "type": "tool_call",
        "tool_name": "mcp__kix__search"
      },
      "hooks": [
        {
          "type": "pre",
          "command": "echo \\"[KIX] Searching knowledge base...\\" >> ~/.kix/audit.log"
        }
      ]
    },
    {
      "matcher": {
        "type": "tool_call",
        "tool_name": "mcp__kix__index"
      },
      "hooks": [
        {
          "type": "post",
          "command": "osascript -e 'display notification \\"Document indexed successfully\\" with title \\"KIX\\"'"
        }
      ]
    },
    {
      "matcher": {
        "type": "tool_call",
        "tool_name": "mcp__kix__index_async"
      },
      "hooks": [
        {
          "type": "post",
          "command": "echo \\"[KIX] Async indexing job started: $TOOL_RESULT\\" >> ~/.kix/jobs.log"
        }
      ]
    }
  ]
}`,
    },
    mcpEvents: {
      title: 'MCP Event Webhooks',
      description: 'Webhook configurations for KIX indexing events',
      content: `{
  "webhooks": {
    "on_index_complete": {
      "url": "https://your-server.com/hooks/kix/indexed",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer \${KIX_WEBHOOK_TOKEN}",
        "Content-Type": "application/json"
      },
      "payload": {
        "event": "index_complete",
        "document_id": "\${document_id}",
        "title": "\${title}",
        "chunks_created": "\${chunks_count}",
        "timestamp": "\${timestamp}"
      }
    },
    "on_search": {
      "url": "https://your-server.com/hooks/kix/search",
      "method": "POST",
      "payload": {
        "event": "search",
        "query": "\${query}",
        "results_count": "\${results.length}",
        "mode": "\${mode}",
        "timestamp": "\${timestamp}"
      }
    }
  }
}`,
    },
  },
  'cursor': {
    claudeHooks: {
      title: 'Cursor Event Hooks',
      description: 'Event hooks for Cursor IDE KIX integration',
      content: `{
  "cursor_hooks": {
    "on_file_save": {
      "condition": "file.extension in ['.md', '.mdx', '.rst']",
      "action": "suggest_index",
      "message": "Index this documentation file to KIX?"
    },
    "on_search_no_results": {
      "action": "suggest_crawl",
      "message": "No results found. Would you like to index relevant documentation?"
    },
    "on_composer_start": {
      "action": "preload_context",
      "search_recent": true,
      "limit": 5
    }
  }
}`,
    },
    mcpEvents: {
      title: 'Cursor Notifications',
      description: 'Notification configuration for KIX events in Cursor',
      content: `{
  "notifications": {
    "index_complete": {
      "enabled": true,
      "type": "toast",
      "message": "Document indexed: \${title}",
      "duration": 3000
    },
    "search_complete": {
      "enabled": false
    },
    "crawl_progress": {
      "enabled": true,
      "type": "progress",
      "show_percentage": true
    }
  }
}`,
    },
  },
  'windsurf': {
    claudeHooks: {
      title: 'Cascade Hooks',
      description: 'Event hooks for Windsurf Cascade integration',
      content: `{
  "cascade_hooks": {
    "before_flow": {
      "action": "check_kix_status",
      "on_failure": "warn_user"
    },
    "on_documentation_request": {
      "action": "auto_search_kix",
      "include_context": true
    },
    "after_code_generation": {
      "action": "verify_against_docs",
      "search_generated_apis": true
    }
  }
}`,
    },
    mcpEvents: {
      title: 'Windsurf Events',
      description: 'Event handlers for KIX in Windsurf',
      content: `{
  "event_handlers": {
    "kix:index_complete": {
      "action": "refresh_suggestions",
      "notify": true
    },
    "kix:search_complete": {
      "action": "update_context_panel",
      "show_sources": true
    }
  }
}`,
    },
  },
  'vscode': {
    claudeHooks: {
      title: 'VS Code Extension Hooks',
      description: 'Extension event hooks for KIX integration',
      content: `{
  "vscode_hooks": {
    "onDidSaveTextDocument": {
      "pattern": "**/*.{md,mdx,rst,txt}",
      "action": "prompt_index",
      "delay": 1000
    },
    "onDidOpenTextDocument": {
      "pattern": "**/*.{ts,tsx,js,jsx,py}",
      "action": "search_related_docs",
      "show_in_sidebar": true
    }
  }
}`,
    },
    mcpEvents: {
      title: 'VS Code Events',
      description: 'VS Code event configuration for KIX',
      content: `{
  "events": {
    "kix.indexComplete": {
      "handler": "showInformationMessage",
      "args": ["Document indexed successfully"]
    },
    "kix.searchComplete": {
      "handler": "updateWebviewPanel",
      "panel": "kixResults"
    }
  }
}`,
    },
  },
};

// ============================================
// COMPONENTS
// ============================================

// Transport badge component
function TransportBadge({
  transport,
  size = 'sm',
  recommended = false,
  showLabel = true,
}: {
  transport: TransportType;
  size?: 'sm' | 'md';
  recommended?: boolean;
  showLabel?: boolean;
}) {
  const config = {
    http: { label: 'HTTP', color: 'cyan', icon: Globe },
    stdio: { label: 'stdio', color: 'violet', icon: Terminal },
    both: { label: 'Both', color: 'emerald', icon: Layers },
  }[transport];

  const Icon = config.icon;

  // Visual hierarchy: solid style for recommended, outline for alternative
  if (recommended) {
    return (
      <span
        className={clsx(
          'inline-flex items-center gap-1 font-medium rounded-full border',
          size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-2.5 py-1 text-sm',
          config.color === 'cyan' && 'bg-cyan-500/20 border-cyan-500/30 text-cyan-300',
          config.color === 'violet' && 'bg-violet-500/20 border-violet-500/30 text-violet-300',
          config.color === 'emerald' && 'bg-emerald-500/20 border-emerald-500/30 text-emerald-300'
        )}
      >
        <Icon className={size === 'sm' ? 'w-3 h-3' : 'w-3.5 h-3.5'} />
        {showLabel && config.label}
        <span className="text-[10px] opacity-70">★</span>
      </span>
    );
  }

  // Alternative transport: outline style with muted colors
  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1 font-medium rounded-full border bg-transparent',
        size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-2.5 py-1 text-sm',
        config.color === 'cyan' && 'border-cyan-500/20 text-cyan-400/70 hover:border-cyan-500/40',
        config.color === 'violet' && 'border-violet-500/20 text-violet-400/70 hover:border-violet-500/40',
        config.color === 'emerald' && 'border-emerald-500/20 text-emerald-400/70 hover:border-emerald-500/40'
      )}
    >
      <Icon className={size === 'sm' ? 'w-3 h-3' : 'w-3.5 h-3.5'} />
      {showLabel && config.label}
    </span>
  );
}

// Component to display all transport badges for a client
function ClientTransportBadges({
  transports,
  size = 'sm',
}: {
  transports: ClientTransports;
  size?: 'sm' | 'md';
}) {
  return (
    <div className="flex items-center gap-1.5">
      {transports.http?.supported && (
        <TransportBadge
          transport="http"
          size={size}
          recommended={transports.http.recommended}
        />
      )}
      {transports.stdio?.supported && (
        <TransportBadge
          transport="stdio"
          size={size}
          recommended={transports.stdio.recommended}
        />
      )}
    </div>
  );
}

// Transport config switcher for clients with multiple transports
function TransportConfigSwitcher({
  client,
  selectedTransport,
  onTransportChange,
}: {
  client: ClientConfig;
  selectedTransport: 'http' | 'stdio';
  onTransportChange: (transport: 'http' | 'stdio') => void;
}) {
  const hasMultiple = hasMultipleTransports(client.transports);

  if (!hasMultiple) {
    return null;
  }

  return (
    <div className="flex items-center gap-4 p-3 bg-slate-800/50 rounded-lg border border-slate-700/50">
      <span className="text-sm text-slate-400">Transport:</span>
      <div className="flex gap-2">
        {client.transports.http?.supported && (
          <button
            onClick={() => onTransportChange('http')}
            className={clsx(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm transition-all',
              selectedTransport === 'http'
                ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/40'
                : 'text-slate-400 hover:text-slate-300 border border-transparent hover:border-slate-700'
            )}
          >
            <Globe className="w-3.5 h-3.5" />
            HTTP
            {client.transports.http.recommended && (
              <span className="text-[10px] text-cyan-400/70">★ Recommended</span>
            )}
          </button>
        )}
        {client.transports.stdio?.supported && (
          <button
            onClick={() => onTransportChange('stdio')}
            className={clsx(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm transition-all',
              selectedTransport === 'stdio'
                ? 'bg-violet-500/20 text-violet-300 border border-violet-500/40'
                : 'text-slate-400 hover:text-slate-300 border border-transparent hover:border-slate-700'
            )}
          >
            <Terminal className="w-3.5 h-3.5" />
            stdio
            {client.transports.stdio.recommended && (
              <span className="text-[10px] text-violet-400/70">★ Recommended</span>
            )}
          </button>
        )}
      </div>
    </div>
  );
}

// Code block component with copy functionality
function CodeBlock({ code, filename, compact = false }: { code: string; filename?: string; compact?: boolean }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      const textarea = document.createElement('textarea');
      textarea.value = code;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="relative group">
      {filename && (
        <div className="flex items-center gap-2 px-4 py-2 bg-slate-800/80 border-b border-slate-700/50 rounded-t-lg">
          <Code2 className="w-3.5 h-3.5 text-slate-500" />
          <span className="text-xs font-mono text-slate-400">{filename}</span>
        </div>
      )}
      <div className={clsx(
        'relative bg-slate-900/80 border border-slate-700/50 overflow-hidden',
        filename ? 'rounded-b-lg border-t-0' : 'rounded-lg'
      )}>
        <button
          onClick={handleCopy}
          className={clsx(
            'absolute top-2 right-2 p-1.5 rounded-md transition-all duration-200',
            'opacity-0 group-hover:opacity-100 focus:opacity-100',
            copied
              ? 'bg-emerald-500/20 text-emerald-400'
              : 'bg-slate-700/50 text-slate-400 hover:bg-slate-700 hover:text-white'
          )}
          title={copied ? 'Copied!' : 'Copy to clipboard'}
        >
          {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
        </button>
        <pre className={clsx('overflow-x-auto', compact ? 'p-3' : 'p-4', !compact && 'max-h-96')}>
          <code className={clsx(
            'font-mono text-slate-300 leading-relaxed whitespace-pre-wrap',
            compact ? 'text-xs' : 'text-sm'
          )}>{code}</code>
        </pre>
      </div>
    </div>
  );
}

// Client/Tool tab component with optional transport badge
function TabButton({
  name,
  icon: Icon,
  isActive,
  onClick,
  transports,
}: {
  name: string;
  icon: React.ComponentType<{ className?: string }>;
  isActive: boolean;
  onClick: () => void;
  transports?: ClientTransports;
}) {
  return (
    <button
      onClick={onClick}
      role="tab"
      aria-selected={isActive}
      className={clsx(
        'flex items-center gap-2 px-3 py-2 rounded-lg font-medium text-sm transition-all duration-200',
        isActive
          ? 'bg-cyan-600 text-white shadow-lg shadow-cyan-500/20'
          : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
      )}
    >
      <Icon className="w-4 h-4" />
      <span className="hidden sm:inline">{name}</span>
      {transports && !isActive && (
        <ClientTransportBadges transports={transports} size="sm" />
      )}
    </button>
  );
}

// Tool card component
function ToolCard({ name, description }: { name: string; description: string }) {
  return (
    <div className="group p-3 rounded-lg bg-slate-800/30 border border-slate-700/30 hover:border-slate-600/50 hover:bg-slate-800/50 transition-all duration-200">
      <code className="text-sm font-mono text-cyan-400 group-hover:text-cyan-300 transition-colors">
        {name}
      </code>
      <p className="text-xs text-slate-500 mt-1 leading-relaxed">{description}</p>
    </div>
  );
}

// Content card component for Agents/Hooks tabs
function ContentCard({
  title,
  description,
  content,
  icon: Icon,
  variant = 'agent',
  filename,
}: {
  title: string;
  description: string;
  content: string;
  icon: React.ComponentType<{ className?: string }>;
  variant?: 'agent' | 'hook';
  filename?: string;
}) {
  return (
    <div className={clsx(
      'p-6 rounded-xl border',
      variant === 'agent'
        ? 'bg-gradient-to-br from-violet-500/5 to-transparent border-violet-500/20'
        : 'bg-gradient-to-br from-amber-500/5 to-transparent border-amber-500/20'
    )}>
      <div className="relative z-10">
        <div className="flex items-start gap-4 mb-4">
          <div className={clsx(
            'p-2 rounded-lg border',
            variant === 'agent'
              ? 'bg-violet-500/10 border-violet-500/20'
              : 'bg-amber-500/10 border-amber-500/20'
          )}>
            <Icon className={clsx(
              'w-5 h-5',
              variant === 'agent' ? 'text-violet-400' : 'text-amber-400'
            )} />
          </div>
          <div>
            <h3 className="text-lg font-semibold text-white">{title}</h3>
            <p className="text-sm text-slate-400 mt-1">{description}</p>
          </div>
        </div>
        <CodeBlock code={content} filename={filename} />
      </div>
    </div>
  );
}

// Main tab navigation component
function MainTabNav({
  activeTab,
  onTabChange,
}: {
  activeTab: 'installation' | 'agents' | 'hooks';
  onTabChange: (tab: 'installation' | 'agents' | 'hooks') => void;
}) {
  const tabs = [
    { id: 'installation' as const, label: 'Installation', icon: Settings2 },
    { id: 'agents' as const, label: 'Agents', icon: Bot },
    { id: 'hooks' as const, label: 'Hooks', icon: Webhook },
  ];

  return (
    <div
      role="tablist"
      aria-label="MCP Configuration sections"
      className="flex flex-wrap gap-1 p-1 bg-slate-800/30 rounded-xl border border-slate-700/30"
    >
      {tabs.map((tab) => {
        const Icon = tab.icon;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={activeTab === tab.id}
            aria-controls={`panel-${tab.id}`}
            id={`tab-${tab.id}`}
            onClick={() => onTabChange(tab.id)}
            className={clsx(
              'flex items-center gap-2 px-4 py-2.5 rounded-lg font-medium text-sm transition-all duration-200',
              activeTab === tab.id
                ? 'bg-slate-700 text-white'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
            )}
          >
            <Icon className="w-4 h-4" />
            <span>{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
}

// Transport Option Card Component
function TransportCard({
  option,
  isSelected,
  onSelect,
}: {
  option: typeof transportOptions[number];
  isSelected: boolean;
  onSelect: () => void;
}) {
  const Icon = option.icon;
  const colorClasses = {
    emerald: {
      bg: 'from-emerald-500/10 to-emerald-500/5',
      border: isSelected ? 'border-emerald-500/50 ring-2 ring-emerald-500/20' : 'border-emerald-500/20 hover:border-emerald-500/40',
      icon: 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400',
      badge: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
    },
    cyan: {
      bg: 'from-cyan-500/10 to-cyan-500/5',
      border: isSelected ? 'border-cyan-500/50 ring-2 ring-cyan-500/20' : 'border-cyan-500/20 hover:border-cyan-500/40',
      icon: 'bg-cyan-500/10 border-cyan-500/20 text-cyan-400',
      badge: 'bg-cyan-500/20 text-cyan-400 border-cyan-500/30',
    },
    violet: {
      bg: 'from-violet-500/10 to-violet-500/5',
      border: isSelected ? 'border-violet-500/50 ring-2 ring-violet-500/20' : 'border-violet-500/20 hover:border-violet-500/40',
      icon: 'bg-violet-500/10 border-violet-500/20 text-violet-400',
      badge: 'bg-violet-500/20 text-violet-400 border-violet-500/30',
    },
  }[option.color as 'emerald' | 'cyan' | 'violet'];

  return (
    <button
      onClick={onSelect}
      className={clsx(
        'relative p-5 rounded-xl border text-left transition-all duration-200',
        'bg-gradient-to-br',
        colorClasses.bg,
        colorClasses.border
      )}
    >
      {option.recommended && (
        <span className="absolute -top-2.5 left-4 px-2 py-0.5 text-xs font-medium rounded-full bg-emerald-500 text-white">
          Recommended
        </span>
      )}

      <div className="flex items-start gap-3 mb-3">
        <div className={clsx('p-2 rounded-lg border', colorClasses.icon)}>
          <Icon className="w-5 h-5" />
        </div>
        <div>
          <h3 className="font-semibold text-white">{option.name}</h3>
          <p className="text-xs text-slate-400">{option.subtitle}</p>
        </div>
        {isSelected && (
          <CheckCircle2 className="w-5 h-5 text-emerald-400 ml-auto" />
        )}
      </div>

      <p className="text-sm text-slate-400 mb-3">{option.description}</p>

      <div className="space-y-1.5">
        {option.features.map((feature, idx) => (
          <div key={idx} className="flex items-center gap-2 text-xs text-slate-500">
            <ChevronRight className="w-3 h-3" />
            <span>{feature}</span>
          </div>
        ))}
      </div>
    </button>
  );
}

// Quick Start Tabbed Section
function QuickStartSection({ selectedTransport }: { selectedTransport: typeof transportOptions[number]['id'] }) {
  const option = transportOptions.find(o => o.id === selectedTransport) || transportOptions[0];

  return (
    <section className="card p-6">
      <div className="flex items-center gap-3 mb-6">
        <div className="p-2 rounded-lg bg-amber-500/10 border border-amber-500/20">
          <Zap className="w-5 h-5 text-amber-400" />
        </div>
        <div>
          <h2 className="text-xl font-semibold text-white">Quick Start</h2>
          <p className="text-sm text-slate-400">Get KIX running in seconds</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Command */}
        <div>
          <h3 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
            <span className="flex items-center justify-center w-5 h-5 rounded-full bg-slate-700 text-xs text-slate-300">1</span>
            Start the server
          </h3>
          <CodeBlock code={option.command} compact />
          {option.altCommand && (
            <p className="text-xs text-slate-500 mt-2">
              Or: <code className="text-cyan-400/80">{option.altCommand}</code>
            </p>
          )}
        </div>

        {/* Running Services */}
        <div>
          <h3 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
            <span className="flex items-center justify-center w-5 h-5 rounded-full bg-slate-700 text-xs text-slate-300">2</span>
            Services running
          </h3>
          <div className="p-4 rounded-lg bg-slate-900/80 border border-slate-700/50 space-y-2">
            {option.ports.map((port, idx) => (
              <div key={idx} className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                <code className="text-xs font-mono text-slate-300">{port}</code>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* MCP Endpoint */}
      {selectedTransport !== 'stdio' && (
        <div className="mt-6 p-4 rounded-lg bg-slate-800/50 border border-slate-700/30">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium text-slate-300">MCP HTTP Endpoint</p>
              <code className="text-sm font-mono text-cyan-400">http://localhost:3000/mcp</code>
            </div>
            <span className="px-2 py-0.5 text-xs rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
              via proxy
            </span>
          </div>
          <p className="text-xs text-slate-500 mt-2">
            Uses web server proxy for CORS handling. Direct: <code className="text-slate-400">http://localhost:3002/mcp</code>
          </p>
        </div>
      )}
    </section>
  );
}

// Installation Tab Content
function InstallationTab({
  activeClient,
  onClientChange,
  selectedTransport,
  onTransportChange,
  configTransport,
  onConfigTransportChange,
}: {
  activeClient: string;
  onClientChange: (client: string) => void;
  selectedTransport: typeof transportOptions[number]['id'];
  onTransportChange: (transport: typeof transportOptions[number]['id']) => void;
  configTransport: Record<string, 'http' | 'stdio'>;
  onConfigTransportChange: (clientId: string, transport: 'http' | 'stdio') => void;
}) {
  const selectedClient = clients.find((c) => c.id === activeClient) || clients[0];
  const currentTransport = configTransport[selectedClient.id] || getPrimaryTransport(selectedClient.transports);

  // Filter troubleshooting based on selected transport
  const filteredTroubleshooting = troubleshooting.filter(
    t => t.transport === 'both' || t.transport === selectedTransport || selectedTransport === 'unified'
  );

  return (
    <div className="space-y-8">
      {/* Transport Selection */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-teal-500/10 border border-teal-500/20">
            <Network className="w-5 h-5 text-teal-400" />
          </div>
          <div>
            <h2 className="text-xl font-semibold text-white">Choose Transport</h2>
            <p className="text-sm text-slate-400">Select how clients connect to KIX</p>
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          {transportOptions.map((option) => (
            <TransportCard
              key={option.id}
              option={option}
              isSelected={selectedTransport === option.id}
              onSelect={() => onTransportChange(option.id)}
            />
          ))}
        </div>
      </section>

      {/* Quick Start */}
      <QuickStartSection selectedTransport={selectedTransport} />

      {/* Client Configuration Section */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-cyan-500/10 border border-cyan-500/20">
            <Server className="w-5 h-5 text-cyan-400" />
          </div>
          <h2 className="text-xl font-semibold text-white">Client Configuration</h2>
        </div>

        {/* Client tabs with transport badges */}
        <div className="flex flex-wrap gap-2 mb-6 p-1 bg-slate-800/30 rounded-xl border border-slate-700/30" role="tablist">
          {clients.map((client) => (
            <TabButton
              key={client.id}
              name={client.name}
              icon={client.icon}
              isActive={activeClient === client.id}
              onClick={() => onClientChange(client.id)}
              transports={client.transports}
            />
          ))}
        </div>

        {/* Selected client config */}
        <div className="space-y-4">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              {(() => {
                const Icon = selectedClient.icon;
                return (
                  <div className="p-2 rounded-lg bg-slate-800 border border-slate-700">
                    <Icon className="w-5 h-5 text-cyan-400" />
                  </div>
                );
              })()}
              <div>
                <h3 className="text-lg font-medium text-white flex items-center gap-2">
                  {selectedClient.name}
                  <ClientTransportBadges transports={selectedClient.transports} size="sm" />
                </h3>
                <p className="text-sm text-slate-400">{selectedClient.description}</p>
              </div>
            </div>
          </div>

          {/* Transport switcher for multi-transport clients */}
          <TransportConfigSwitcher
            client={selectedClient}
            selectedTransport={currentTransport}
            onTransportChange={(transport) => onConfigTransportChange(selectedClient.id, transport)}
          />

          {/* Transport-specific note */}
          {selectedClient.transports[currentTransport]?.note && (
            <p className="text-xs text-slate-500 italic flex items-center gap-1">
              <ChevronRight className="w-3 h-3" />
              {selectedClient.transports[currentTransport]?.note}
            </p>
          )}

          {/* stdio requirement warning for stdio-only clients */}
          {!selectedClient.transports.http?.supported && selectedClient.transports.stdio?.supported && (
            <div className="flex items-start gap-3 p-4 rounded-lg bg-violet-500/10 border border-violet-500/20">
              <Terminal className="w-5 h-5 text-violet-400 flex-shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-violet-300">stdio Transport Required</p>
                <p className="text-sm text-violet-400/80 mt-1">
                  Server must be started with <code className="bg-violet-500/20 px-1 rounded">--stdio</code> flag
                </p>
              </div>
            </div>
          )}

          {/* CLI Command Option */}
          {selectedClient.cliCommands?.[currentTransport] && (
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-slate-300 flex items-center gap-2">
                <Terminal className="w-4 h-4 text-emerald-400" />
                Quick Add (CLI)
              </h4>
              <CodeBlock code={selectedClient.cliCommands[currentTransport]!} compact />
            </div>
          )}

          {/* Manual Configuration */}
          {selectedClient.configs[currentTransport] && (
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-slate-300 flex items-center gap-2">
                <Code2 className="w-4 h-4 text-cyan-400" />
                Configuration File
              </h4>
              <CodeBlock code={selectedClient.configs[currentTransport]!} filename={selectedClient.configPath} />
            </div>
          )}

          {selectedClient.configPathAlt && (
            <p className="text-xs text-slate-500 flex items-center gap-1">
              <ChevronRight className="w-3 h-3" />
              Windows: <code className="text-slate-400">{selectedClient.configPathAlt}</code>
            </p>
          )}

          {selectedClient.note && (
            <div className="flex items-start gap-2 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20">
              <AlertCircle className="w-4 h-4 text-amber-400 flex-shrink-0 mt-0.5" />
              <p className="text-sm text-amber-300">{selectedClient.note}</p>
            </div>
          )}
        </div>
      </section>

      {/* Available Tools Section */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-teal-500/10 border border-teal-500/20">
            <Plug2 className="w-5 h-5 text-teal-400" />
          </div>
          <h2 className="text-xl font-semibold text-white">Available Tools</h2>
          <span className="ml-auto text-sm text-slate-500">8 tools</span>
        </div>

        <div className="space-y-6">
          {toolCategories.map((category) => {
            const Icon = category.icon;
            const colorClasses = {
              cyan: 'bg-cyan-500/10 border-cyan-500/20 text-cyan-400',
              violet: 'bg-violet-500/10 border-violet-500/20 text-violet-400',
              amber: 'bg-amber-500/10 border-amber-500/20 text-amber-400',
            }[category.color];

            return (
              <div key={category.name}>
                <div className="flex items-center gap-2 mb-3">
                  <div className={clsx('p-1.5 rounded-lg border', colorClasses)}>
                    <Icon className="w-4 h-4" />
                  </div>
                  <h3 className="font-medium text-white">{category.name}</h3>
                  <span className="text-xs text-slate-500">({category.tools.length})</span>
                </div>
                <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
                  {category.tools.map((tool) => (
                    <ToolCard key={tool.name} name={tool.name} description={tool.description} />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Troubleshooting Section */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-red-500/10 border border-red-500/20">
            <AlertCircle className="w-5 h-5 text-red-400" />
          </div>
          <h2 className="text-xl font-semibold text-white">Troubleshooting</h2>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          {filteredTroubleshooting.map((item, index) => (
            <div
              key={index}
              className="p-4 rounded-lg bg-slate-800/30 border border-slate-700/30"
            >
              <div className="flex items-center gap-2 mb-2">
                <h4 className="font-medium text-white text-sm">{item.issue}</h4>
                {item.transport !== 'both' && (
                  <TransportBadge transport={item.transport as TransportType} size="sm" />
                )}
              </div>
              <p className="text-sm text-slate-400 leading-relaxed">{item.solution}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Additional Resources */}
      <section className="card p-6">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="font-medium text-white">Need more help?</h3>
            <p className="text-sm text-slate-400 mt-1">
              Check the MCP specification or report issues on GitHub
            </p>
          </div>
          <a
            href="https://modelcontextprotocol.io"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-slate-800 text-slate-300 hover:text-white hover:bg-slate-700 transition-colors text-sm"
          >
            MCP Docs
            <ExternalLink className="w-4 h-4" />
          </a>
        </div>
      </section>
    </div>
  );
}

// Agents Tab Content
function AgentsTab({ activeTool, onToolChange }: {
  activeTool: string;
  onToolChange: (tool: string) => void;
}) {
  const content = agentContent[activeTool] || agentContent['claude-code'];
  const selectedTool = agentTools.find((t) => t.id === activeTool) || agentTools[0];

  return (
    <div className="space-y-6">
      {/* Tool selector */}
      <div className="flex flex-wrap gap-2 p-1 bg-slate-800/30 rounded-xl border border-slate-700/30" role="tablist">
        {agentTools.map((tool) => (
          <TabButton
            key={tool.id}
            name={tool.name}
            icon={tool.icon}
            isActive={activeTool === tool.id}
            onClick={() => onToolChange(tool.id)}
          />
        ))}
      </div>

      {/* Tool description */}
      <div className="flex items-center gap-3 p-4 rounded-lg bg-slate-800/30 border border-slate-700/30">
        {(() => {
          const Icon = selectedTool.icon;
          return <Icon className="w-5 h-5 text-cyan-400" />;
        })()}
        <div>
          <h3 className="font-medium text-white">{selectedTool.name} Agent Configuration</h3>
          <p className="text-sm text-slate-400">{selectedTool.description}</p>
        </div>
      </div>

      {/* Content cards */}
      <div className="grid gap-6">
        <ContentCard
          title={content.claudeMd.title}
          description={content.claudeMd.description}
          content={content.claudeMd.content}
          icon={FileText}
          variant="agent"
          filename={activeTool === 'claude-code' ? 'CLAUDE.md' : activeTool === 'cursor' ? '.cursorrules' : 'config.md'}
        />

        <ContentCard
          title={content.workflow.title}
          description={content.workflow.description}
          content={content.workflow.content}
          icon={Workflow}
          variant="agent"
          filename="workflow.json"
        />

        <ContentCard
          title={content.systemPrompt.title}
          description={content.systemPrompt.description}
          content={content.systemPrompt.content}
          icon={MessageSquare}
          variant="agent"
          filename="system_prompt.md"
        />
      </div>
    </div>
  );
}

// Hooks Tab Content
function HooksTab({ activeTool, onToolChange }: {
  activeTool: string;
  onToolChange: (tool: string) => void;
}) {
  const content = hooksContent[activeTool] || hooksContent['claude-code'];
  const selectedTool = agentTools.find((t) => t.id === activeTool) || agentTools[0];

  return (
    <div className="space-y-6">
      {/* Tool selector */}
      <div className="flex flex-wrap gap-2 p-1 bg-slate-800/30 rounded-xl border border-slate-700/30" role="tablist">
        {agentTools.map((tool) => (
          <TabButton
            key={tool.id}
            name={tool.name}
            icon={tool.icon}
            isActive={activeTool === tool.id}
            onClick={() => onToolChange(tool.id)}
          />
        ))}
      </div>

      {/* Tool description */}
      <div className="flex items-center gap-3 p-4 rounded-lg bg-slate-800/30 border border-slate-700/30">
        {(() => {
          const Icon = selectedTool.icon;
          return <Icon className="w-5 h-5 text-amber-400" />;
        })()}
        <div>
          <h3 className="font-medium text-white">{selectedTool.name} Hooks Configuration</h3>
          <p className="text-sm text-slate-400">Event hooks and webhooks for {selectedTool.name}</p>
        </div>
      </div>

      {/* Content cards */}
      <div className="grid gap-6">
        <ContentCard
          title={content.claudeHooks.title}
          description={content.claudeHooks.description}
          content={content.claudeHooks.content}
          icon={Bell}
          variant="hook"
          filename={activeTool === 'claude-code' ? '.claude/hooks.json' : 'hooks.json'}
        />

        <ContentCard
          title={content.mcpEvents.title}
          description={content.mcpEvents.description}
          content={content.mcpEvents.content}
          icon={Globe}
          variant="hook"
          filename="webhooks.json"
        />
      </div>
    </div>
  );
}

// ============================================
// MAIN COMPONENT
// ============================================

export default function MCPDocs() {
  const [searchParams, setSearchParams] = useSearchParams();

  // Get active states from URL params
  const activeMainTab = (searchParams.get('tab') as 'installation' | 'agents' | 'hooks') || 'installation';
  const activeClient = searchParams.get('client') || 'claude-desktop';
  const selectedTransport = (searchParams.get('transport') as typeof transportOptions[number]['id']) || 'unified';

  // Config transport state - tracks which transport config to show for each client
  const [configTransport, setConfigTransport] = useState<Record<string, 'http' | 'stdio'>>(() => {
    // Initialize with recommended transport for each client
    const initial: Record<string, 'http' | 'stdio'> = {};
    clients.forEach(client => {
      initial[client.id] = getPrimaryTransport(client.transports);
    });
    return initial;
  });

  // Update URL when states change
  const setActiveMainTab = useCallback((tab: 'installation' | 'agents' | 'hooks') => {
    setSearchParams({ tab, client: activeClient, transport: selectedTransport });
  }, [activeClient, selectedTransport, setSearchParams]);

  const setActiveClient = useCallback((client: string) => {
    setSearchParams({ tab: activeMainTab, client, transport: selectedTransport });
  }, [activeMainTab, selectedTransport, setSearchParams]);

  const setSelectedTransport = useCallback((transport: typeof transportOptions[number]['id']) => {
    setSearchParams({ tab: activeMainTab, client: activeClient, transport });
  }, [activeMainTab, activeClient, setSearchParams]);

  // Update config transport for a specific client
  const handleConfigTransportChange = useCallback((clientId: string, transport: 'http' | 'stdio') => {
    setConfigTransport(prev => ({ ...prev, [clientId]: transport }));
  }, []);

  return (
    <div className="space-y-8 pb-12">
      {/* Hero Section */}
      <div className="relative overflow-hidden">
        {/* Decorative background elements */}
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          <div className="absolute top-0 right-0 w-96 h-96 bg-cyan-500/5 rounded-full blur-3xl" />
          <div className="absolute bottom-0 left-0 w-64 h-64 bg-violet-500/5 rounded-full blur-3xl" />
        </div>

        <div className="relative">
          <div className="flex items-center gap-4 mb-4">
            <div className="p-3 rounded-xl bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20">
              <Plug2 className="w-8 h-8 text-cyan-400" />
            </div>
            <div>
              <h1 className="text-3xl font-bold text-white">MCP Integration</h1>
              <p className="text-slate-400">Model Context Protocol</p>
            </div>
          </div>

          <p className="text-lg text-slate-300 max-w-2xl leading-relaxed">
            Connect your AI assistant to KIX for semantic search and knowledge management.
          </p>

          {/* Quick stats */}
          <div className="flex flex-wrap gap-4 mt-5">
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-800/50 border border-slate-700/50 text-sm">
              <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span className="text-slate-300">8 tools</span>
            </div>
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-800/50 border border-slate-700/50 text-sm">
              <Globe className="w-3.5 h-3.5 text-cyan-400" />
              <span className="text-slate-300">HTTP transport</span>
            </div>
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-800/50 border border-slate-700/50 text-sm">
              <Terminal className="w-3.5 h-3.5 text-violet-400" />
              <span className="text-slate-300">stdio transport</span>
            </div>
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-800/50 border border-slate-700/50 text-sm">
              <Search className="w-3.5 h-3.5 text-amber-400" />
              <span className="text-slate-300">Hybrid search</span>
            </div>
          </div>
        </div>
      </div>

      {/* Main Tab Navigation */}
      <MainTabNav activeTab={activeMainTab} onTabChange={setActiveMainTab} />

      {/* Tab Content */}
      <div
        role="tabpanel"
        id={`panel-${activeMainTab}`}
        aria-labelledby={`tab-${activeMainTab}`}
        tabIndex={0}
      >
        {activeMainTab === 'installation' && (
          <InstallationTab
            activeClient={activeClient}
            onClientChange={setActiveClient}
            selectedTransport={selectedTransport}
            onTransportChange={setSelectedTransport}
            configTransport={configTransport}
            onConfigTransportChange={handleConfigTransportChange}
          />
        )}
        {activeMainTab === 'agents' && (
          <AgentsTab
            activeTool={activeClient}
            onToolChange={setActiveClient}
          />
        )}
        {activeMainTab === 'hooks' && (
          <HooksTab
            activeTool={activeClient}
            onToolChange={setActiveClient}
          />
        )}
      </div>
    </div>
  );
}

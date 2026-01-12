import { useState } from 'react';
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
} from 'lucide-react';
import clsx from 'clsx';

// Client configuration data
const clients = [
  {
    id: 'claude-code',
    name: 'Claude Code',
    icon: Terminal,
    description: 'Anthropic CLI tool',
    cliCommand: 'claude mcp add kix --transport http http://localhost:3000/mcp',
    configPath: '~/.claude/settings.json',
    config: `{
  "mcpServers": {
    "kix": {
      "type": "http",
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
  },
  {
    id: 'claude-desktop',
    name: 'Claude Desktop',
    icon: Monitor,
    description: 'Desktop application',
    cliCommand: 'claude mcp add kix -- kix serve',
    configPath: '~/Library/Application Support/Claude/claude_desktop_config.json',
    configPathAlt: '%APPDATA%\\Claude\\claude_desktop_config.json (Windows)',
    config: `{
  "mcpServers": {
    "kix": {
      "command": "kix",
      "args": ["serve"]
    }
  }
}`,
    note: 'Uses stdio transport for direct binary integration',
  },
  {
    id: 'cursor',
    name: 'Cursor',
    icon: Code2,
    description: 'AI-first code editor',
    cliCommand: 'cursor --add-mcp kix http://localhost:3000/mcp',
    configPath: '.cursor/mcp.json',
    config: `{
  "mcpServers": {
    "kix": {
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
  },
  {
    id: 'windsurf',
    name: 'Windsurf',
    icon: Sparkles,
    description: 'Codeium IDE',
    cliCommand: 'windsurf --add-mcp kix http://localhost:3000/mcp',
    configPath: '~/.codeium/windsurf/mcp_config.json',
    config: `{
  "mcpServers": {
    "kix": {
      "serverUrl": "http://localhost:3000/mcp"
    }
  }
}`,
  },
  {
    id: 'vscode',
    name: 'VS Code',
    icon: Code2,
    description: 'With MCP extension',
    configPath: '.vscode/settings.json',
    config: `{
  "mcp.servers": {
    "kix": {
      "url": "http://localhost:3000/mcp"
    }
  }
}`,
    note: 'Requires MCP extension to be installed',
  },
];

// Tool categories
const toolCategories = [
  {
    name: 'Search',
    icon: Search,
    color: 'cyan',
    tools: [
      { name: 'search_patterns', description: 'Semantic search using natural language with hybrid vector + full-text search' },
      { name: 'get_pattern', description: 'Retrieve a specific pattern by its exact name' },
      { name: 'list_patterns', description: 'List all patterns, optionally filtered by category or type' },
      { name: 'find_related', description: 'Find patterns related to a given pattern' },
      { name: 'search_by_problem', description: 'Find patterns that solve specific integration problems' },
      { name: 'search_by_technology', description: 'Find patterns for specific technologies (Camel, Spring, etc.)' },
    ],
  },
  {
    name: 'Analysis',
    icon: Brain,
    color: 'violet',
    tools: [
      { name: 'explain_pattern', description: 'Get detailed explanation with usage, implementation, or tradeoffs focus' },
      { name: 'compare_patterns', description: 'Side-by-side comparison of two patterns' },
      { name: 'get_category_overview', description: 'Overview of all patterns in a category' },
      { name: 'suggest_architecture', description: 'Suggest patterns for system architectures with constraints' },
      { name: 'pattern_sequence', description: 'Show typical pattern sequences and flows in pipelines' },
    ],
  },
  {
    name: 'Indexing',
    icon: Database,
    color: 'amber',
    tools: [
      { name: 'index_document', description: 'Index a single document from text, file path, or URL' },
      { name: 'index_batch', description: 'Index multiple documents in one operation (max 50)' },
      { name: 'delete_document', description: 'Delete a document and its chunks by ID' },
      { name: 'get_index_status', description: 'Get indexing statistics and system health' },
    ],
  },
];

// Troubleshooting items
const troubleshooting = [
  {
    issue: 'Connection refused',
    solution: 'Ensure KIX server is running. Start with ./run.sh or kix serve-http --port 3002',
  },
  {
    issue: 'CORS errors',
    solution: 'Use the proxied endpoint at port 3000 instead of direct connection to port 3002',
  },
  {
    issue: 'Tools not appearing',
    solution: 'Restart your AI client after making configuration changes. Check config file syntax.',
  },
  {
    issue: 'Slow responses',
    solution: 'First request may be slow as embedding models initialize. Subsequent requests are faster.',
  },
];

// Code block component with copy functionality
function CodeBlock({ code, filename }: { code: string; filename?: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
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
            'absolute top-3 right-3 p-2 rounded-lg transition-all duration-200',
            'opacity-0 group-hover:opacity-100 focus:opacity-100',
            copied
              ? 'bg-emerald-500/20 text-emerald-400'
              : 'bg-slate-700/50 text-slate-400 hover:bg-slate-700 hover:text-white'
          )}
          title={copied ? 'Copied!' : 'Copy to clipboard'}
        >
          {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
        </button>
        <pre className="p-4 overflow-x-auto">
          <code className="text-sm font-mono text-slate-300 leading-relaxed">{code}</code>
        </pre>
      </div>
    </div>
  );
}

// Client tab component
function ClientTab({
  client,
  isActive,
  onClick,
}: {
  client: typeof clients[0];
  isActive: boolean;
  onClick: () => void;
}) {
  const Icon = client.icon;
  return (
    <button
      onClick={onClick}
      className={clsx(
        'flex items-center gap-2 px-4 py-2.5 rounded-lg font-medium text-sm transition-all duration-200',
        isActive
          ? 'bg-cyan-600 text-white shadow-lg shadow-cyan-500/20'
          : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
      )}
    >
      <Icon className="w-4 h-4" />
      <span className="hidden sm:inline">{client.name}</span>
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

export default function MCPDocs() {
  const [activeClient, setActiveClient] = useState(clients[0].id);
  const selectedClient = clients.find((c) => c.id === activeClient) || clients[0];

  return (
    <div className="space-y-12 pb-12">
      {/* Hero Section */}
      <div className="relative overflow-hidden">
        {/* Decorative background elements */}
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          <div className="absolute top-0 right-0 w-96 h-96 bg-cyan-500/5 rounded-full blur-3xl" />
          <div className="absolute bottom-0 left-0 w-64 h-64 bg-teal-500/5 rounded-full blur-3xl" />
        </div>

        <div className="relative">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-3 rounded-xl bg-gradient-to-br from-cyan-500/20 to-teal-500/20 border border-cyan-500/20">
              <Plug2 className="w-8 h-8 text-cyan-400" />
            </div>
            <div>
              <h1 className="text-4xl font-bold text-white">MCP Integration</h1>
              <p className="text-slate-400 mt-1">Model Context Protocol</p>
            </div>
          </div>

          <p className="text-lg text-slate-300 max-w-2xl leading-relaxed">
            Connect your AI assistant to KIX for semantic search and knowledge management.
            MCP enables AI tools to query your indexed content using natural language.
          </p>

          {/* Quick stats */}
          <div className="flex flex-wrap gap-6 mt-6">
            <div className="flex items-center gap-2 text-sm">
              <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span className="text-slate-400">16 tools available</span>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <div className="w-2 h-2 rounded-full bg-cyan-400" />
              <span className="text-slate-400">HTTP & stdio transports</span>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <div className="w-2 h-2 rounded-full bg-violet-400" />
              <span className="text-slate-400">Hybrid search (vector + FTS)</span>
            </div>
          </div>
        </div>
      </div>

      {/* Quick Start Section */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-amber-500/10 border border-amber-500/20">
            <Zap className="w-5 h-5 text-amber-400" />
          </div>
          <h2 className="text-xl font-semibold text-white">Quick Start</h2>
        </div>

        <div className="grid gap-6 md:grid-cols-2">
          <div>
            <h3 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
              <span className="flex items-center justify-center w-5 h-5 rounded-full bg-slate-700 text-xs text-slate-300">1</span>
              Start the KIX server
            </h3>
            <CodeBlock code="./run.sh" />
            <p className="text-xs text-slate-500 mt-2">
              Or manually: <code className="text-cyan-400/80">kix serve-http --port 3002</code>
            </p>
          </div>

          <div>
            <h3 className="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
              <span className="flex items-center justify-center w-5 h-5 rounded-full bg-slate-700 text-xs text-slate-300">2</span>
              MCP Endpoint
            </h3>
            <div className="p-4 rounded-lg bg-slate-900/80 border border-slate-700/50">
              <div className="flex items-center justify-between">
                <code className="text-sm font-mono text-cyan-400">http://localhost:3000/mcp</code>
                <span className="px-2 py-0.5 text-xs rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                  recommended
                </span>
              </div>
              <p className="text-xs text-slate-500 mt-2">
                Uses the web server proxy for CORS handling
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Client Configuration Section */}
      <section className="card p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="p-2 rounded-lg bg-cyan-500/10 border border-cyan-500/20">
            <Terminal className="w-5 h-5 text-cyan-400" />
          </div>
          <h2 className="text-xl font-semibold text-white">Client Configuration</h2>
        </div>

        {/* Client tabs */}
        <div className="flex flex-wrap gap-2 mb-6 p-1 bg-slate-800/30 rounded-xl border border-slate-700/30">
          {clients.map((client) => (
            <ClientTab
              key={client.id}
              client={client}
              isActive={activeClient === client.id}
              onClick={() => setActiveClient(client.id)}
            />
          ))}
        </div>

        {/* Selected client config */}
        <div className="space-y-4">
          <div className="flex items-start justify-between">
            <div>
              <h3 className="text-lg font-medium text-white flex items-center gap-2">
                {(() => {
                  const Icon = selectedClient.icon;
                  return <Icon className="w-5 h-5 text-cyan-400" />;
                })()}
                {selectedClient.name}
              </h3>
              <p className="text-sm text-slate-400">{selectedClient.description}</p>
            </div>
          </div>

          {/* CLI Command Option */}
          {selectedClient.cliCommand && (
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-slate-300 flex items-center gap-2">
                <Terminal className="w-4 h-4 text-emerald-400" />
                Quick Add (CLI)
              </h4>
              <CodeBlock code={selectedClient.cliCommand} />
            </div>
          )}

          {/* Manual Configuration */}
          <div className="space-y-2">
            <h4 className="text-sm font-medium text-slate-300 flex items-center gap-2">
              <Code2 className="w-4 h-4 text-cyan-400" />
              Manual Configuration
            </h4>
            <CodeBlock code={selectedClient.config} filename={selectedClient.configPath} />
          </div>

          {selectedClient.configPathAlt && (
            <p className="text-xs text-slate-500 flex items-center gap-1">
              <ChevronRight className="w-3 h-3" />
              Windows: <code className="text-slate-400">{selectedClient.configPathAlt}</code>
            </p>
          )}

          {selectedClient.note && (
            <div className="flex items-start gap-2 p-3 rounded-lg bg-violet-500/10 border border-violet-500/20">
              <AlertCircle className="w-4 h-4 text-violet-400 flex-shrink-0 mt-0.5" />
              <p className="text-sm text-violet-300">{selectedClient.note}</p>
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
          <span className="ml-auto text-sm text-slate-500">16 tools</span>
        </div>

        <div className="space-y-8">
          {toolCategories.map((category) => {
            const Icon = category.icon;
            const colorClasses = {
              cyan: 'bg-cyan-500/10 border-cyan-500/20 text-cyan-400',
              violet: 'bg-violet-500/10 border-violet-500/20 text-violet-400',
              amber: 'bg-amber-500/10 border-amber-500/20 text-amber-400',
            }[category.color];

            return (
              <div key={category.name}>
                <div className="flex items-center gap-2 mb-4">
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

        <div className="grid gap-4 sm:grid-cols-2">
          {troubleshooting.map((item, index) => (
            <div
              key={index}
              className="p-4 rounded-lg bg-slate-800/30 border border-slate-700/30"
            >
              <h4 className="font-medium text-white text-sm mb-2">{item.issue}</h4>
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

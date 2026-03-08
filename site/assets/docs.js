(function () {
  const lang = document.documentElement.lang && document.documentElement.lang.startsWith("zh") ? "zh" : "en";
  const STORAGE_THEME_KEY = "mosaic_docs_theme";

  const index = {
    en: [
      { title: "Overview", url: "index.html", desc: "Project scope and first commands", keywords: "overview intro start" },
      { title: "Guide Hub", url: "guide.html", desc: "Choose quickstart, learning path, module deep dives, or operations", keywords: "guide hub path quickstart learning models sessions policy agents operations" },
      { title: "Quickstart", url: "quickstart.html", desc: "10-minute first success path", keywords: "quickstart 10 minute setup ask chat" },
      { title: "Learning Path", url: "learning-path.html", desc: "Detailed stage-by-stage learning from basic to advanced", keywords: "learning path step by step beginner advanced" },
      { title: "Models & Profiles", url: "models-profiles.html", desc: "Profile setup, aliases, fallback chain, and model routing", keywords: "models profile configure alias fallback resolve set" },
      { title: "Sessions Guide", url: "sessions.html", desc: "Session lifecycle create show resume clear with ask/chat", keywords: "session list show resume clear ask chat --session" },
      { title: "Policy Guide", url: "policy.html", desc: "Approvals and sandbox policy setup with command checks", keywords: "approvals sandbox safety allowlist confirm deny restricted standard elevated" },
      { title: "Azure Ops Playbook", url: "playbook-azure-ops.html", desc: "One-run Azure playbook from setup to regression gate", keywords: "azure playbook setup models channels gateway policy regression" },
      { title: "Agents Guide", url: "agents.html", desc: "Module tutorial for agents and route management", keywords: "agents route add update default ask chat" },
      { title: "Channels Guide", url: "channels.html", desc: "Operate terminal, Slack, and Telegram channels step by step", keywords: "channels add list test send logs capabilities resolve rotate-token-env" },
      { title: "Memory Guide", url: "memory.html", desc: "Index and search project context step by step", keywords: "memory index search status clear" },
      { title: "Knowledge Guide", url: "knowledge.html", desc: "Ingest markdown/http/mcp sources and ask with references", keywords: "knowledge ingest search ask local_md http mcp namespace" },
      { title: "Plugins Guide", url: "plugins.html", desc: "Plugin lifecycle install check enable run remove", keywords: "plugins install enable disable run doctor" },
      { title: "Skills Guide", url: "skills.html", desc: "Skills lifecycle install check info remove", keywords: "skills install check info remove" },
      { title: "Gateway Guide", url: "gateway.html", desc: "Gateway lifecycle and call interface tutorial", keywords: "gateway install start status health probe call" },
      { title: "Gateway Call API", url: "gateway-call.html", desc: "Detailed contract and automation usage of gateway call", keywords: "gateway call params json request response probe" },
      { title: "Regression Guide", url: "regression.html", desc: "From zero to release gate regression workflow", keywords: "regression test smoke release gate tutorial script" },
      { title: "Operations", url: "operations.html", desc: "Production runbook for channels gateway policy diagnostics", keywords: "operations runbook channels gateway approvals sandbox logs" },
      { title: "Install", url: "install.html", desc: "Install on macOS Linux Windows", keywords: "brew powershell install.sh" },
      { title: "Configure", url: "configure.html", desc: "Provider model profile setup", keywords: "azure openai base-url api-key-env" },
      { title: "Troubleshooting", url: "troubleshooting.html", desc: "Common failures and diagnosis commands", keywords: "404 path health doctor logs" }
    ],
    zh: [
      { title: "总览", url: "index.html", desc: "项目范围与首批命令", keywords: "总览 入门" },
      { title: "路径导航", url: "guide.html", desc: "在入门、分阶段学习、模块专题、生产运维间选择路径", keywords: "路径 导航 10分钟 分阶段 models sessions policy agents 运维" },
      { title: "10 分钟上手", url: "quickstart.html", desc: "最短路径跑通 setup ask chat", keywords: "10分钟 上手 setup ask chat" },
      { title: "分阶段学习", url: "learning-path.html", desc: "从最简单到复杂的阶段化教程", keywords: "学习路径 分阶段 一步一步 入门 进阶" },
      { title: "Models 与 Profiles", url: "models-profiles.html", desc: "profile 配置、alias/fallback 与模型路由", keywords: "models profile configure alias fallback 模型 路由" },
      { title: "Sessions 教程", url: "sessions.html", desc: "会话创建、查看、恢复、清理全流程", keywords: "session list show resume clear ask chat 会话" },
      { title: "策略教程", url: "policy.html", desc: "approvals 与 sandbox 策略配置和判定检查", keywords: "approvals sandbox safety allowlist confirm deny 策略" },
      { title: "Azure 运维剧本", url: "playbook-azure-ops.html", desc: "从 Azure 配置到回归门禁的一次性剧本", keywords: "azure 运维 剧本 setup models channels gateway policy 回归" },
      { title: "Agents 专项", url: "agents.html", desc: "agents 与路由管理的详细教程", keywords: "agents 路由 add update default ask chat" },
      { title: "Channels 教程", url: "channels.html", desc: "terminal、Slack、Telegram 通道分步实操", keywords: "channels add list test send logs capabilities resolve" },
      { title: "Memory 教程", url: "memory.html", desc: "索引与检索模块分步教程", keywords: "memory index search status clear" },
      { title: "Knowledge 教程", url: "knowledge.html", desc: "采集本地/HTTP/MCP 数据源并基于检索问答", keywords: "knowledge ingest search ask local_md http mcp namespace" },
      { title: "Plugins 教程", url: "plugins.html", desc: "插件安装检查启停执行与移除", keywords: "plugins install enable disable run doctor" },
      { title: "Skills 教程", url: "skills.html", desc: "技能安装检查信息与移除", keywords: "skills install check info remove" },
      { title: "Gateway 教程", url: "gateway.html", desc: "网关生命周期与调用接口实操", keywords: "gateway install start status health probe call" },
      { title: "Gateway Call API", url: "gateway-call.html", desc: "gateway call 请求契约与自动化调用模式", keywords: "gateway call params json 协议 返回" },
      { title: "回归测试教程", url: "regression.html", desc: "从零到发版门禁的回归测试路径", keywords: "回归 测试 冒烟 发版 门禁 教程 脚本" },
      { title: "生产运维", url: "operations.html", desc: "日常稳定运行的通道、网关、策略与诊断", keywords: "运维 通道 网关 approvals sandbox logs" },
      { title: "安装", url: "install.html", desc: "macOS Linux Windows 安装方式", keywords: "brew powershell 安装脚本" },
      { title: "配置", url: "configure.html", desc: "provider model profile 配置", keywords: "azure openai base-url api-key-env" },
      { title: "排障", url: "troubleshooting.html", desc: "常见故障定位与修复", keywords: "404 doctor health logs" }
    ]
  };

  function slugify(input) {
    return input
      .toLowerCase()
      .trim()
      .replace(/[\s\u3000]+/g, "-")
      .replace(/[^\w\u4e00-\u9fa5-]/g, "")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, "");
  }

  function preferredTheme() {
    const stored = window.localStorage.getItem(STORAGE_THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function setTheme(mode) {
    document.documentElement.setAttribute("data-theme", mode);
    window.localStorage.setItem(STORAGE_THEME_KEY, mode);
    const btn = document.getElementById("theme-toggle");
    if (btn) {
      const isDark = mode === "dark";
      btn.textContent = isDark ? "☀" : "☾";
      btn.title = lang === "zh" ? "切换主题" : "Toggle theme";
      btn.setAttribute("aria-label", lang === "zh" ? "切换主题" : "Toggle theme");
    }
  }

  function setupThemeToggle() {
    const btn = document.getElementById("theme-toggle");
    if (!btn) return;

    setTheme(preferredTheme());
    btn.addEventListener("click", () => {
      const current = document.documentElement.getAttribute("data-theme") || "light";
      setTheme(current === "dark" ? "light" : "dark");
    });
  }

  function buildToc() {
    const container = document.getElementById("doc-toc-list");
    const tocRoot = document.getElementById("doc-toc");
    const article = document.querySelector(".doc");
    if (!container || !article || !tocRoot) return;

    const headings = Array.from(article.querySelectorAll("h2, h3"));
    if (!headings.length) {
      tocRoot.style.display = "none";
      return;
    }

    const used = new Set();
    const links = [];

    headings.forEach((heading) => {
      const level = heading.tagName.toLowerCase();
      const base = heading.id || slugify(heading.textContent || "section");
      let id = base || "section";
      let count = 2;
      while (used.has(id)) {
        id = `${base}-${count}`;
        count += 1;
      }
      used.add(id);
      heading.id = id;

      if (!heading.querySelector(".heading-anchor")) {
        const anchor = document.createElement("a");
        anchor.className = "heading-anchor";
        anchor.href = `#${id}`;
        anchor.textContent = "#";
        heading.prepend(anchor);
      }

      const a = document.createElement("a");
      a.className = `toc-link ${level === "h3" ? "toc-sub" : ""}`;
      a.href = `#${id}`;
      a.textContent = heading.textContent ? heading.textContent.replace(/^#\s*/, "") : id;
      container.appendChild(a);
      links.push({ id, link: a });
    });

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const item = links.find((x) => x.id === entry.target.id);
          if (!item) return;
          if (entry.isIntersecting) {
            links.forEach((x) => x.link.classList.remove("active"));
            item.link.classList.add("active");
          }
        });
      },
      {
        rootMargin: "-90px 0px -70% 0px",
        threshold: [0, 1]
      }
    );

    headings.forEach((h) => observer.observe(h));
  }

  function setupSearch() {
    const input = document.getElementById("doc-search");
    const panel = document.getElementById("doc-search-results");
    if (!input || !panel) return;

    const list = index[lang] || index.en;

    function hide() {
      panel.classList.remove("visible");
      panel.innerHTML = "";
    }

    function render(query) {
      const q = query.trim().toLowerCase();
      if (!q) {
        hide();
        return;
      }

      const hits = list
        .map((item) => {
          const haystack = `${item.title} ${item.desc} ${item.keywords}`.toLowerCase();
          return {
            item,
            score: haystack.includes(q) ? (item.title.toLowerCase().includes(q) ? 3 : 1) : 0
          };
        })
        .filter((x) => x.score > 0)
        .sort((a, b) => b.score - a.score)
        .slice(0, 12);

      if (!hits.length) {
        panel.innerHTML = `<div class="search-empty">${lang === "zh" ? "没有匹配结果" : "No matching results"}</div>`;
        panel.classList.add("visible");
        return;
      }

      panel.innerHTML = hits
        .map(
          ({ item }) =>
            `<a class="search-item" href="${item.url}"><div class="search-title">${item.title}</div><div class="search-desc">${item.desc}</div></a>`
        )
        .join("");
      panel.classList.add("visible");
    }

    input.addEventListener("input", () => render(input.value));
    input.addEventListener("focus", () => render(input.value));
    document.addEventListener("keydown", (event) => {
      if (event.key === "/" && document.activeElement !== input) {
        const tag = document.activeElement && document.activeElement.tagName;
        if (tag !== "INPUT" && tag !== "TEXTAREA") {
          event.preventDefault();
          input.focus();
        }
      }
      if (event.key === "Escape") {
        hide();
      }
    });

    document.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (!panel.contains(target) && target !== input) hide();
    });
  }

  function setupCodeCopy() {
    const blocks = Array.from(document.querySelectorAll("pre"));
    blocks.forEach((pre) => {
      const code = pre.querySelector("code");
      if (!code) return;

      pre.classList.add("has-copy");
      const btn = document.createElement("button");
      btn.className = "copy-btn";
      btn.type = "button";
      btn.textContent = lang === "zh" ? "复制" : "Copy";

      btn.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(code.innerText);
          const old = btn.textContent;
          btn.textContent = lang === "zh" ? "已复制" : "Copied";
          window.setTimeout(() => {
            btn.textContent = old;
          }, 1200);
        } catch (_err) {
          btn.textContent = lang === "zh" ? "失败" : "Failed";
          window.setTimeout(() => {
            btn.textContent = lang === "zh" ? "复制" : "Copy";
          }, 1200);
        }
      });

      pre.appendChild(btn);
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    setupThemeToggle();
    buildToc();
    setupSearch();
    setupCodeCopy();
  });
})();

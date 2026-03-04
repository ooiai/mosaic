(function () {
  const lang = document.documentElement.lang && document.documentElement.lang.startsWith("zh") ? "zh" : "en";

  const index = {
    en: [
      { title: "Overview", url: "index.html", desc: "Project scope and first commands", keywords: "overview intro start" },
      { title: "Usage Guide", url: "guide.html", desc: "Step-by-step path from install to daily usage", keywords: "guide quickstart" },
      { title: "Install", url: "install.html", desc: "Install on macOS Linux Windows", keywords: "brew powershell install.sh" },
      { title: "Configure", url: "configure.html", desc: "Provider model profile setup", keywords: "azure openai base-url api-key-env" },
      { title: "Troubleshooting", url: "troubleshooting.html", desc: "Common failures and diagnosis commands", keywords: "404 path health doctor logs" }
    ],
    zh: [
      { title: "总览", url: "index.html", desc: "项目范围与首批命令", keywords: "总览 入门" },
      { title: "使用路径", url: "guide.html", desc: "从安装到日常使用", keywords: "教程 上手" },
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

      const a = document.createElement("a");
      a.className = `toc-link ${level === "h3" ? "toc-sub" : ""}`;
      a.href = `#${id}`;
      a.textContent = heading.textContent || id;
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
          return { item, score: haystack.includes(q) ? (item.title.toLowerCase().includes(q) ? 2 : 1) : 0 };
        })
        .filter((x) => x.score > 0)
        .sort((a, b) => b.score - a.score)
        .slice(0, 8);

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

  document.addEventListener("DOMContentLoaded", () => {
    buildToc();
    setupSearch();
  });
})();

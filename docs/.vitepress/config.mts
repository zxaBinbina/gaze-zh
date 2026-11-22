// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

import { defineVersionedConfig } from "@viteplus/versions";
import { createHighlighter } from "shiki";
import fs from "node:fs";
import path from "node:path";

const INSTALL_CMD = "curl -fsSL https://zxabinbina.cc.cd/projects/gaze/install.sh | sh";

const highlightedInstall = await createHighlighter({
  themes: ["github-light", "github-dark"],
  langs: ["bash"],
}).then((hl) =>
  hl.codeToHtml(INSTALL_CMD, {
    lang: "bash",
    themes: { light: "github-light", dark: "github-dark" },
    defaultColor: false,
  }),
);

export default defineVersionedConfig({
  vite: {
    plugins: [
      {
        name: "install-highlight",
        resolveId(id) {
          if (id === "virtual:install-highlight") return id;
        },
        load(id) {
          if (id === "virtual:install-highlight")
            return `export const html = ${JSON.stringify(highlightedInstall)}; export const command = ${JSON.stringify(INSTALL_CMD)};`;
        },
      },
    ],
  },
  buildEnd(siteConfig) {
    const current指南 = path.join(siteConfig.outDir, "main", "guide");
    const legacy指南 = path.join(siteConfig.outDir, "guide");
    fs.mkdirSync(legacy指南, { recursive: true });
    for (const filename of fs.readdirSync(current指南).filter((name) => name.endsWith(".html"))) {
      const destination = path.join(legacy指南, filename);
      const target = `${siteConfig.base ?? "/"}main/guide/${filename}`;
      fs.writeFileSync(destination, `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="refresh" content="0;url=${target}"><link rel="canonical" href="${target}"><title>页面已迁移</title></head><body><script>location.replace(${JSON.stringify(target)} + location.search + location.hash)</script><a href="${target}">打开当前指南</a></body></html>\n`);
    }
  },
  ignoreDeadLinks: false,
  lang: "zh-CN",
  title: "Gaze 中文版",
  description: "Linux 本地人脸认证",
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" }],
  ],
  lastUpdated: true,
  sitemap: {
    hostname: "https://zxabinbina.cc.cd/projects/gaze",
  },
  versionsConfig: {
    current: "main",
    versionSwitcher: false,
  },
  themeConfig: {
    returnToTopLabel: "返回顶部",
    sidebarMenuLabel: "菜单",
    darkModeSwitchLabel: "切换深色模式",
    lastUpdated: { text: "最后更新" },
    docFooter: { prev: "上一篇", next: "下一篇" },
    search: {
      provider: "local",
    },
    outline: {
      level: [2, 3],
    },
    editLink: {
      pattern: "https://github.com/zxaBinbina/gaze-zh/edit/main/docs/:path",
      text: "在 GitHub 上编辑此页",
    },
    logo: "/favicon.svg",
    nav: [
      { text: "首页", link: "/" },
      { text: "指南", link: "/guide/getting-started" },
      { component: "VersionSwitcher" },
    ],

    sidebar: [
      {
        text: "指南",
        items: [
          { text: "快速开始", link: "/guide/getting-started" },
          { text: "安装", link: "/guide/installation" },
          { text: "Nix & NixOS", link: "/guide/nixos" },
          { text: "开发", link: "/guide/development" },
          { text: "参与贡献", link: "/guide/contributing" },
          {
            text: "认证",
            items: [
              { text: "PAM", link: "/guide/pam" },
              { text: "GNOME 扩展", link: "/guide/gnome" },
              { text: "Cinnamon 扩展", link: "/guide/cinnamon" },
              { text: "KDE Plasma", link: "/guide/kde" },
              { text: "Hyprland (hyprlock)", link: "/guide/hyprland" },
              { text: "Omarchy", link: "/guide/omarchy" },
              { text: "LightDM", link: "/guide/lightdm" },
              { text: "greetd", link: "/guide/greetd" },
              { text: "控制台登录（TTY）", link: "/guide/console" },
            ],
          },
          { text: "GUI 指南", link: "/guide/gui" },
          { text: "CLI 指南", link: "/guide/cli" },
          { text: "配置", link: "/guide/configuration" },
          { text: "硬件加速", link: "/guide/acceleration" },
          { text: "卸载", link: "/guide/uninstallation" },
          { text: "故障排查", link: "/guide/troubleshooting" },
          { text: "工作原理", link: "/guide/how-it-works" },
          { text: "功能对比", link: "/guide/comparison" },
        ],
      },
    ],

    socialLinks: [
      { icon: "github", link: "https://github.com/zxaBinbina/gaze-zh" },
    ],
  },
});

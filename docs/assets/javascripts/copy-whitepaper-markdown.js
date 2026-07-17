(function () {
  "use strict";

  const buttonId = "postfiat-copy-whitepaper-markdown";
  const rawMarkdownPath = "../assets/raw/whitepaper.md.txt";

  function isWhitepaperV2Page() {
    const normalized = window.location.pathname.replace(/\/+$/, "");
    return (
      normalized.endsWith("/whitepaper") ||
      normalized.endsWith("/whitepaper/index.html")
    );
  }

  function setButtonState(button, label, state) {
    button.textContent = label;
    button.dataset.copyState = state;
  }

  async function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return;
    }

    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    textarea.style.top = "0";
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand("copy");
    } finally {
      document.body.removeChild(textarea);
    }
  }

  async function copyMarkdown(button) {
    const original = button.textContent;
    button.disabled = true;
    setButtonState(button, "Copying...", "pending");

    try {
      const response = await fetch(new URL(rawMarkdownPath, window.location.href), {
        cache: "no-store",
      });
      if (!response.ok) {
        throw new Error("raw markdown fetch failed");
      }
      await copyText(await response.text());
      setButtonState(button, "Copied", "success");
    } catch (error) {
      setButtonState(button, "Copy failed", "error");
      window.setTimeout(() => {
        button.disabled = false;
        setButtonState(button, original, "ready");
      }, 1800);
      return;
    }

    window.setTimeout(() => {
      button.disabled = false;
      setButtonState(button, original, "ready");
    }, 1800);
  }

  function installCopyButton() {
    if (!isWhitepaperV2Page()) {
      return;
    }
    if (document.getElementById(buttonId)) {
      return;
    }

    const content = document.querySelector(".md-content__inner");
    if (!content) {
      return;
    }

    const heading = content.querySelector("h1");
    const toolbar = document.createElement("div");
    toolbar.className = "whitepaper-copy-toolbar";

    const button = document.createElement("button");
    button.id = buttonId;
    button.type = "button";
    button.className = "whitepaper-copy-button";
    button.title = "Copy the Markdown source for this whitepaper";
    button.setAttribute("aria-label", "Copy Whitepaper Markdown");
    setButtonState(button, "Copy Markdown", "ready");
    button.addEventListener("click", () => copyMarkdown(button));

    toolbar.appendChild(button);

    if (heading && heading.parentNode) {
      heading.insertAdjacentElement("afterend", toolbar);
    } else {
      content.insertAdjacentElement("afterbegin", toolbar);
    }
  }

  if (window.document$ && typeof window.document$.subscribe === "function") {
    window.document$.subscribe(installCopyButton);
  } else if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", installCopyButton);
  } else {
    installCopyButton();
  }
})();

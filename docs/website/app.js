/* Progressive enhancement only: the page works without JavaScript. */
(function () {
  "use strict";

  var tabs = Array.prototype.slice.call(document.querySelectorAll('[role="tab"][data-tab]'));
  var panels = Array.prototype.slice.call(document.querySelectorAll('[role="tabpanel"][data-panel]'));
  var liveRegion = document.getElementById("copy-status");

  function activate(tab, focus) {
    var selected = tab.getAttribute("data-tab");
    tabs.forEach(function (button) {
      var active = button === tab;
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-selected", active ? "true" : "false");
      button.tabIndex = active ? 0 : -1;
    });
    panels.forEach(function (panel) {
      panel.hidden = panel.getAttribute("data-panel") !== selected;
    });
    if (focus) tab.focus();
  }

  tabs.forEach(function (tab, index) {
    tab.addEventListener("click", function () { activate(tab, false); });
    tab.addEventListener("keydown", function (event) {
      var offset = event.key === "ArrowRight" ? 1 : (event.key === "ArrowLeft" ? -1 : 0);
      if (offset !== 0) {
        event.preventDefault();
        activate(tabs[(index + offset + tabs.length) % tabs.length], true);
      } else if (event.key === "Home") {
        event.preventDefault();
        activate(tabs[0], true);
      } else if (event.key === "End") {
        event.preventDefault();
        activate(tabs[tabs.length - 1], true);
      }
    });
  });

  function fallbackCopy(value) {
    var input = document.createElement("textarea");
    input.value = value;
    input.setAttribute("readonly", "");
    input.style.position = "fixed";
    input.style.left = "-9999px";
    document.body.appendChild(input);
    input.select();
    var copied = false;
    try { copied = document.execCommand("copy"); } catch (_) { copied = false; }
    document.body.removeChild(input);
    return copied;
  }

  Array.prototype.forEach.call(document.querySelectorAll("[data-copy]"), function (button) {
    var initialText = button.textContent;
    var resetHandle;
    button.addEventListener("click", async function () {
      var block = button.closest(".code-wrap");
      var code = block && block.querySelector("pre code");
      if (!code) return;
      var text = code.textContent.replace(/\s+$/, "") + "\n";
      var success = false;
      try {
        if (navigator.clipboard && window.isSecureContext) {
          await navigator.clipboard.writeText(text);
          success = true;
        } else {
          success = fallbackCopy(text);
        }
      } catch (_) {
        success = fallbackCopy(text);
      }
      if (resetHandle) window.clearTimeout(resetHandle);
      button.classList.toggle("is-copied", success);
      button.textContent = success ? "Copied ✓" : "Select code to copy";
      if (liveRegion) liveRegion.textContent = success ? "Code copied to clipboard" : "Clipboard unavailable; select the code manually";
      resetHandle = window.setTimeout(function () {
        button.classList.remove("is-copied");
        button.textContent = initialText;
      }, 2100);
    });
  });
})();

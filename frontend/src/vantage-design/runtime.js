/* Classic script, also safe to import from client bundles or SSR. No network. */
(function () {
  if (typeof window === "undefined" || typeof document === "undefined" || window.VantageDesign) return;
  var root = document.documentElement;
  function stored(key, fallback) { try { return localStorage.getItem(key) || fallback; } catch (_) { return fallback; } }
  function parentAppearance() {
    try { if (window.parent !== window && window.parent.VantageDesign) return window.parent.VantageDesign.current(); } catch (_) { /* standalone/cross-origin */ }
    return null;
  }
  function current() { return { theme: root.dataset.uuuuiTheme === "dark" ? "dark" : "light", density: root.dataset.vantageDensity === "compact" ? "compact" : "comfortable" }; }
  function apply(value, save) {
    var next = parentAppearance() || value;
    var theme = next.theme === "dark" ? "dark" : "light";
    var density = next.density === "compact" ? "compact" : "comfortable";
    root.dataset.theme = theme; root.dataset.uuuuiTheme = theme;
    root.dataset.vantageDensity = density; root.classList.toggle("dark", theme === "dark"); root.style.colorScheme = theme;
    if (save) { try { localStorage.setItem("uuuui-theme", theme); localStorage.setItem("vantage-density", density); } catch (_) { /* storage denied */ } }
    window.dispatchEvent(new CustomEvent("vantage-appearance", { detail: { theme: theme, density: density } }));
    document.querySelectorAll("iframe").forEach(function (frame) { try { if (frame.contentWindow.VantageDesign) frame.contentWindow.VantageDesign.sync(); } catch (_) { /* sandbox/report */ } });
  }
  window.VantageDesign = {
    current: current,
    setTheme: function (theme) { apply({ theme: theme, density: current().density }, true); },
    setDensity: function (density) { apply({ theme: current().theme, density: density }, true); },
    sync: function () { apply(parentAppearance() || { theme: stored("uuuui-theme", "light"), density: stored("vantage-density", "comfortable") }, false); }
  };
  window.VantageDesign.sync();
  window.addEventListener("storage", function (event) { if (!event.key || event.key === "uuuui-theme" || event.key === "vantage-density") window.VantageDesign.sync(); });
  window.addEventListener("uuuui-theme", function (event) { var state = current(); if (event.detail === "dark" || event.detail === "light") apply({ theme: event.detail, density: state.density }, false); });
})();

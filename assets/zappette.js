// zappette connection health.
//
// Procedure calls that fail at the network layer throw inside topcoat's
// generated handlers and die as unhandled rejections — the page would sit
// there looking alive while every button is dead. This script listens for
// those rejections plus the browser's network events, and flips <body>
// between the online and offline states the stylesheet reacts to.
//
// While offline it probes /healthz (1s backoff up to 5s, only while the tab
// is visible) until the server answers. If the answering binary reports a
// different build id than the one this page was rendered with, the page's
// procedure ids are stale, so it reloads itself exactly once.
(function () {
  "use strict";

  var meta = document.querySelector('meta[name="zappette-build"]');
  var buildId = meta ? meta.getAttribute("content") : "";
  var timer = null;
  var backoffMs = 1000;

  function isOffline() {
    return document.body.classList.contains("offline");
  }

  function scheduleProbe(delayMs) {
    if (timer !== null) return;
    timer = setTimeout(function () {
      timer = null;
      probe();
    }, delayMs);
  }

  function goOffline() {
    backoffMs = 1000;
    document.body.classList.add("offline");
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    scheduleProbe(0);
  }

  function goOnline() {
    document.body.classList.remove("offline");
    backoffMs = 1000;
  }

  function probe() {
    if (document.hidden) return; // visibilitychange restarts probing
    var abort = new AbortController();
    var timeout = setTimeout(function () {
      abort.abort();
    }, 3000);
    fetch("/healthz", { cache: "no-store", signal: abort.signal })
      .then(function (response) {
        if (!response.ok) throw new Error("healthz " + response.status);
        return response.json();
      })
      .then(function (health) {
        if (buildId !== "" && health.build !== buildId) {
          location.reload();
          return;
        }
        goOnline();
      })
      .catch(function () {
        document.body.classList.add("offline");
        backoffMs = Math.min(backoffMs * 2, 5000);
        scheduleProbe(backoffMs);
      })
      .finally(function () {
        clearTimeout(timeout);
      });
  }

  // A failed procedure call or shard fetch surfaces here; the same failure
  // may also come through as an `error` event, so both land in goOffline.
  window.addEventListener("unhandledrejection", goOffline);
  window.addEventListener("offline", goOffline);
  window.addEventListener("online", function () {
    scheduleProbe(0);
  });
  document.addEventListener("visibilitychange", function () {
    if (!document.hidden) scheduleProbe(0);
  });

  // Host context: focused window + browser state, one poll for both lines
  // (the /focus response carries them in fixed order, staged on the way to a
  // /context payload). On failure the lines just go stale — the health script
  // owns the offline state, and the next successful poll catches up.
  var focusText = document.getElementById("focus-text");
  var browserText = document.getElementById("browser-text");
  if (focusText !== null) {
    var refreshFocus = function () {
      if (document.hidden) return;
      fetch("/focus", { cache: "no-store" })
        .then(function (response) {
          if (!response.ok) throw new Error("focus " + response.status);
          return response.text();
        })
        .then(function (text) {
          var lines = text.split("\n");
          focusText.textContent = lines[0] || "";
          if (browserText !== null) browserText.textContent = lines[1] || "";
        })
        .catch(function () {});
    };
    refreshFocus();
    setInterval(refreshFocus, 2000);
    document.addEventListener("visibilitychange", function () {
      if (!document.hidden) refreshFocus();
    });
  }
})();

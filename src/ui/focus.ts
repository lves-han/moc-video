// Keep browser focus as the single source of truth; native form controls and
// dialogs retain their own keyboard behavior.
const interactive = 'a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]';
const rowSelector = '[data-focus-row],.film-grid,.primary-nav,.utility-nav';
const visible = (element: HTMLElement) => element.getClientRects().length > 0 && getComputedStyle(element).visibility !== "hidden" && !element.closest('[inert],[aria-hidden="true"]');
function items(root: ParentNode) {
  return [...root.querySelectorAll<HTMLElement>(interactive)].filter(element => visible(element) && !element.matches('.card-title'));
}
export function installFocusNavigation(root: HTMLElement, escape: () => boolean) {
  const keydown = (event: KeyboardEvent) => {
    if (!matchMedia("(min-width: 960px)").matches || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
    const target = document.activeElement as HTMLElement | null;
    if (!target || target.closest('[role="dialog"],.n-modal,video,video-player')) return;
    if (event.key === "Escape") {
      if (escape()) event.preventDefault();
      return;
    }
    if (target.matches('input,select,textarea,[contenteditable="true"]')) return;
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    const candidates = items(root);
    if (!candidates.length) return;
    let next: HTMLElement | undefined;
    const row = target.closest<HTMLElement>(rowSelector);
    const horizontal = event.key === "ArrowLeft" || event.key === "ArrowRight";
    if (horizontal && row && !row.matches("nav")) {
      const peers = items(row), index = peers.indexOf(target);
      next = peers[Math.max(0, Math.min(peers.length - 1, index + (event.key === "ArrowRight" ? 1 : -1)))];
    } else {
      const origin = target.getBoundingClientRect();
      const x = origin.x + origin.width / 2, y = origin.y + origin.height / 2;
      const direction = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
      const ranked = candidates.filter(candidate => candidate !== target && (horizontal || !row || !row.contains(candidate) || row.matches("nav")))
        .map(candidate => {
          const r = candidate.getBoundingClientRect(), dx = r.x + r.width / 2 - x, dy = r.y + r.height / 2 - y;
          return { candidate, along: (horizontal ? dx : dy) * direction, across: Math.abs(horizontal ? dy : dx) };
        }).filter(item => item.along > 1)
        .sort((a, b) => a.along + a.across * 3 - b.along - b.across * 3);
      next = ranked[0]?.candidate;
    }
    next ??= candidates.includes(target) ? target : candidates[0];
    event.preventDefault();
    next?.focus({ preventScroll: true });
    next?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
  };
  root.addEventListener("keydown", keydown);
  return () => root.removeEventListener("keydown", keydown);
}

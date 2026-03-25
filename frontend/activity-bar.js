function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function createActivityBar({ container, items, onSelect }) {
  let active = items[0]?.id ?? null;

  function render() {
    container.innerHTML = `
      <div class="activity-brand" aria-hidden="true">ES</div>
      <div class="activity-list">
        ${items
          .map(
            (item) => `
              <button
                type="button"
                class="activity-button ${item.id === active ? "active" : ""}"
                data-activity="${escapeHtml(item.id)}"
                title="${escapeHtml(item.label)}"
                aria-label="${escapeHtml(item.label)}"
              >
                <span class="activity-button-label">${escapeHtml(item.short)}</span>
                <span class="activity-button-text">${escapeHtml(item.text)}</span>
              </button>
            `
          )
          .join("")}
      </div>
    `;

    container.querySelectorAll("[data-activity]").forEach((button) => {
      button.addEventListener("click", () => {
        active = button.dataset.activity ?? active;
        render();
        onSelect(active);
      });
    });
  }

  render();

  return {
    setActive(next) {
      active = next;
      render();
    },
    getActive() {
      return active;
    }
  };
}

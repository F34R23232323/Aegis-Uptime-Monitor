/** Enhance a native select without changing its form value or Svelte binding. */
export function customSelect(select: HTMLSelectElement) {
	const root = document.createElement('div');
	root.className = 'custom-select';
	select.parentNode?.insertBefore(root, select);
	root.appendChild(select);
	select.classList.add('custom-select-native');
	select.tabIndex = -1;
	const accessibleName =
		select.getAttribute('aria-label') ||
		select.closest('label')?.firstChild?.textContent?.trim() ||
		'Select option';
	const trigger = document.createElement('button');
	trigger.type = 'button';
	trigger.className = 'custom-select-trigger';
	trigger.setAttribute('aria-haspopup', 'listbox');
	const menu = document.createElement('div');
	menu.className = 'custom-select-menu';
	menu.setAttribute('role', 'listbox');
	menu.hidden = true;
	const menuId = `custom-select-${crypto.randomUUID()}`;
	menu.id = menuId;
	trigger.setAttribute('aria-controls', menuId);
	root.append(trigger, menu);
	let active = 0;
	let search = '';
	let searchTimer: ReturnType<typeof setTimeout> | undefined;
	function options() {
		return [...select.options].filter((option) => !option.disabled);
	}
	function sync() {
		const selected = select.selectedOptions[0];
		trigger.textContent = selected?.textContent || 'Select…';
		trigger.disabled = select.disabled;
		trigger.setAttribute('aria-expanded', String(!menu.hidden));
		trigger.setAttribute('aria-label', accessibleName);
	}
	function render() {
		menu.replaceChildren();
		options().forEach((option, index) => {
			const item = document.createElement('button');
			item.type = 'button';
			item.className = 'custom-select-option';
			item.setAttribute('role', 'option');
			item.setAttribute('aria-selected', String(option.selected));
			item.tabIndex = index === active ? 0 : -1;
			item.textContent = option.textContent;
			item.addEventListener('click', () => choose(option.value));
			menu.appendChild(item);
		});
	}
	function close() {
		menu.hidden = true;
		sync();
	}
	function open(index?: number) {
		if (select.disabled) return;
		active =
			index ??
			Math.max(
				0,
				options().findIndex((option) => option.selected)
			);
		render();
		menu.hidden = false;
		sync();
		(menu.children[active] as HTMLElement | undefined)?.focus();
	}
	function choose(value: string) {
		select.value = value;
		select.dispatchEvent(new Event('change', { bubbles: true }));
		close();
		trigger.focus();
	}
	function onKey(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			close();
			trigger.focus();
			return;
		}
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			if (menu.hidden) open();
			else (menu.children[active] as HTMLElement | undefined)?.click();
			return;
		}
		const available = options();
		if (!available.length) return;
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || event.key === 'Home' || event.key === 'End') {
			event.preventDefault();
			if (menu.hidden) {
				open();
				return;
			}
			active =
				event.key === 'Home'
					? 0
					: event.key === 'End'
						? available.length - 1
						: (active + (event.key === 'ArrowDown' ? 1 : -1) + available.length) % available.length;
			render();
			(menu.children[active] as HTMLElement | undefined)?.focus();
		} else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
			search += event.key.toLocaleLowerCase();
			clearTimeout(searchTimer);
			searchTimer = setTimeout(() => (search = ''), 600);
			const match = available.findIndex((option) =>
				option.textContent?.toLocaleLowerCase().startsWith(search)
			);
			if (match >= 0) {
				active = match;
				if (menu.hidden) open(match);
				else {
					render();
					(menu.children[active] as HTMLElement | undefined)?.focus();
				}
			}
		}
	}
	function outside(event: PointerEvent) {
		if (!root.contains(event.target as Node)) close();
	}
	function focusOut() {
		queueMicrotask(() => {
			if (!root.contains(document.activeElement)) close();
		});
	}
	trigger.addEventListener('click', () => (menu.hidden ? open() : close()));
	root.addEventListener('keydown', onKey);
	root.addEventListener('focusout', focusOut);
	document.addEventListener('pointerdown', outside);
	select.addEventListener('change', sync);
	const observer = new MutationObserver(sync);
	observer.observe(select, { attributes: true, childList: true, subtree: true });
	sync();
	return {
		destroy() {
			clearTimeout(searchTimer);
			observer.disconnect();
			document.removeEventListener('pointerdown', outside);
			root.removeEventListener('keydown', onKey);
			root.removeEventListener('focusout', focusOut);
			select.removeEventListener('change', sync);
			root.parentNode?.insertBefore(select, root);
			root.remove();
			select.classList.remove('custom-select-native');
			select.removeAttribute('tabindex');
		}
	};
}

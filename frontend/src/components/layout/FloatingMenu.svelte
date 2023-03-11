<script lang="ts" context="module">
	export type MenuDirection = "Top" | "Bottom" | "Left" | "Right" | "TopLeft" | "TopRight" | "BottomLeft" | "BottomRight" | "Center";
	export type MenuType = "Popover" | "Dropdown" | "Dialog" | "Cursor";
</script>

<script lang="ts">
	import { afterUpdate, createEventDispatcher, tick } from "svelte";

	import LayoutCol from "@/components/layout/LayoutCol.svelte";

	const POINTER_STRAY_DISTANCE = 100;

	// emits: ["update:open", "naturalWidth"],
	const dispatch = createEventDispatcher<{ open: boolean; naturalWidth: number }>();

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let open: boolean;
	export let type: MenuType;
	export let direction: MenuDirection = "Bottom";
	export let windowEdgeMargin = 6;
	export let scrollableY = false;
	export let minWidth = 0;
	export let escapeCloses = true;
	export let strayCloses = true;

	let tail: HTMLDivElement;
	let self: HTMLDivElement;
	let floatingMenuContainer: HTMLDivElement;
	let floatingMenuContent: LayoutCol;

	// The resize observer is attached to the floating menu container, which is the zero-height div of the width of the parent element's floating menu spawner.
	// Since CSS doesn't let us make the floating menu (with `position: fixed`) have a 100% width of this container, we need to use JS to observe its size and
	// tell the floating menu content to use it as a min-width so the floating menu is at least the width of the parent element's floating menu spawner.
	// This is the opposite concern of the natural width measurement system, which gets the natural width of the floating menu content in order for the
	// spawner widget to optionally set its min-size to the floating menu's natural width.
	let containerResizeObserver = new ResizeObserver((entries: ResizeObserverEntry[]) => {
		resizeObserverCallback(entries);
	});
	let wasOpen = open;
	let measuringOngoing = false;
	let measuringOngoingGuard = false;
	let minWidthParentWidth = 0;
	let pointerStillDown = false;
	let workspaceBounds = new DOMRect();
	let floatingMenuBounds = new DOMRect();
	let floatingMenuContentBounds = new DOMRect();

	$: minWidthStyleValue = measuringOngoing ? "0" : `${Math.max(minWidth, minWidthParentWidth)}px`;
	$: displayTail = open && type === "Popover";
	$: displayContainer = open || measuringOngoing;
	$: extraClasses = Object.entries(classes)
		.flatMap((classAndState) => (classAndState[1] ? [classAndState[0]] : []))
		.join(" ");
	$: extraStyles = Object.entries(styles)
		.flatMap((styleAndValue) => (styleAndValue[1] !== undefined ? [`${styleAndValue[0]}: ${styleAndValue[1]};`] : []))
		.join(" ");

	$: watchOpenChange(open);

	// Called only when `open` is changed from outside this component
	async function watchOpenChange(isOpen: boolean) {
		// Switching from closed to open
		if (isOpen && !wasOpen) {
			// TODO: Close any other floating menus that may already be open, which can happen using tab navigation and Enter/Space Bar to open

			// Close floating menu if pointer strays far enough away
			window.addEventListener("pointermove", pointerMoveHandler);
			// Close floating menu if esc is pressed
			window.addEventListener("keydown", keyDownHandler);
			// Close floating menu if pointer is outside (but within stray distance)
			window.addEventListener("pointerdown", pointerDownHandler);
			// Cancel the subsequent click event to prevent the floating menu from reopening if the floating menu's button is the click event target
			window.addEventListener("pointerup", pointerUpHandler);

			// Floating menu min-width resize observer

			await tick();

			// Start a new observation of the now-open floating menu
			containerResizeObserver.disconnect();
			containerResizeObserver.observe(floatingMenuContainer);
		}

		// Switching from open to closed
		if (!isOpen && wasOpen) {
			// Clean up observation of the now-closed floating menu
			containerResizeObserver.disconnect();

			window.removeEventListener("pointermove", pointerMoveHandler);
			window.removeEventListener("keydown", keyDownHandler);
			window.removeEventListener("pointerdown", pointerDownHandler);
			// The `pointerup` event is removed in `pointerMoveHandler()` and `pointerDownHandler()`
		}

		// Now that we're done reading the old state, update it to the current state for next time
		wasOpen = isOpen;
	}

	// Gets the client bounds of the elements and apply relevant styles to them
	// TODO: Use DOM attribute bindings more whilst not causing recursive updates
	afterUpdate(() => {
		// Turning measuring on and off both causes the component to change, which causes the `afterUpdate()` Svelte event to fire extraneous times (hurting performance and sometimes causing an infinite loop)
		if (!measuringOngoingGuard) positionAndStyleFloatingMenu();
	});

	function resizeObserverCallback(entries: ResizeObserverEntry[]) {
		minWidthParentWidth = entries[0].contentRect.width;
	}

	function positionAndStyleFloatingMenu() {
		if (type === "Cursor") return;

		const workspace = document.querySelector("[data-workspace]");

		if (!workspace || !self || !floatingMenuContainer || !floatingMenuContent) return;

		workspaceBounds = workspace.getBoundingClientRect();
		floatingMenuBounds = self.getBoundingClientRect();
		const floatingMenuContainerBounds = floatingMenuContainer.getBoundingClientRect();
		floatingMenuContentBounds = floatingMenuContent.div().getBoundingClientRect();

		const inParentFloatingMenu = Boolean(floatingMenuContainer.closest("[data-floating-menu-content]"));

		if (!inParentFloatingMenu) {
			// Required to correctly position content when scrolled (it has a `position: fixed` to prevent clipping)
			// We use `.style` on a div (instead of a style DOM attribute binding) because the binding causes the `afterUpdate()` hook to call the function we're in recursively forever
			const tailOffset = type === "Popover" ? 10 : 0;
			if (direction === "Bottom") floatingMenuContent.div().style.top = `${tailOffset + floatingMenuBounds.top}px`;
			if (direction === "Top") floatingMenuContent.div().style.bottom = `${tailOffset + floatingMenuBounds.bottom}px`;
			if (direction === "Right") floatingMenuContent.div().style.left = `${tailOffset + floatingMenuBounds.left}px`;
			if (direction === "Left") floatingMenuContent.div().style.right = `${tailOffset + floatingMenuBounds.right}px`;

			// Required to correctly position tail when scrolled (it has a `position: fixed` to prevent clipping)
			// We use `.style` on a div (instead of a style DOM attribute binding) because the binding causes the `afterUpdate()` hook to call the function we're in recursively forever
			if (tail && direction === "Bottom") tail.style.top = `${floatingMenuBounds.top}px`;
			if (tail && direction === "Top") tail.style.bottom = `${floatingMenuBounds.bottom}px`;
			if (tail && direction === "Right") tail.style.left = `${floatingMenuBounds.left}px`;
			if (tail && direction === "Left") tail.style.right = `${floatingMenuBounds.right}px`;
		}

		type Edge = "Top" | "Bottom" | "Left" | "Right";
		let zeroedBorderVertical: Edge | undefined;
		let zeroedBorderHorizontal: Edge | undefined;

		if (direction === "Top" || direction === "Bottom") {
			zeroedBorderVertical = direction === "Top" ? "Bottom" : "Top";

			// We use `.style` on a div (instead of a style DOM attribute binding) because the binding causes the `afterUpdate()` hook to call the function we're in recursively forever
			if (floatingMenuContentBounds.left - windowEdgeMargin <= workspaceBounds.left) {
				floatingMenuContent.div().style.left = `${windowEdgeMargin}px`;
				if (workspaceBounds.left + floatingMenuContainerBounds.left === 12) zeroedBorderHorizontal = "Left";
			}
			if (floatingMenuContentBounds.right + windowEdgeMargin >= workspaceBounds.right) {
				floatingMenuContent.div().style.right = `${windowEdgeMargin}px`;
				if (workspaceBounds.right - floatingMenuContainerBounds.right === 12) zeroedBorderHorizontal = "Right";
			}
		}
		if (direction === "Left" || direction === "Right") {
			zeroedBorderHorizontal = direction === "Left" ? "Right" : "Left";

			// We use `.style` on a div (instead of a style DOM attribute binding) because the binding causes the `afterUpdate()` hook to call the function we're in recursively forever
			if (floatingMenuContentBounds.top - windowEdgeMargin <= workspaceBounds.top) {
				floatingMenuContent.div().style.top = `${windowEdgeMargin}px`;
				if (workspaceBounds.top + floatingMenuContainerBounds.top === 12) zeroedBorderVertical = "Top";
			}
			if (floatingMenuContentBounds.bottom + windowEdgeMargin >= workspaceBounds.bottom) {
				floatingMenuContent.div().style.bottom = `${windowEdgeMargin}px`;
				if (workspaceBounds.bottom - floatingMenuContainerBounds.bottom === 12) zeroedBorderVertical = "Bottom";
			}
		}

		// Remove the rounded corner from the content where the tail perfectly meets the corner
		if (type === "Popover" && windowEdgeMargin === 6 && zeroedBorderVertical && zeroedBorderHorizontal) {
			// We use `.style` on a div (instead of a style DOM attribute binding) because the binding causes the `afterUpdate()` hook to call the function we're in recursively forever
			switch (`${zeroedBorderVertical}${zeroedBorderHorizontal}`) {
				case "TopLeft":
					floatingMenuContent.div().style.borderTopLeftRadius = "0";
					break;
				case "TopRight":
					floatingMenuContent.div().style.borderTopRightRadius = "0";
					break;
				case "BottomLeft":
					floatingMenuContent.div().style.borderBottomLeftRadius = "0";
					break;
				case "BottomRight":
					floatingMenuContent.div().style.borderBottomRightRadius = "0";
					break;
				default:
					break;
			}
		}
	}

	export function div(): HTMLDivElement {
		return self;
	}

	// To be called by the parent component. Measures the actual width of the floating menu content element and returns it in a promise.
	export async function measureAndEmitNaturalWidth(): Promise<void> {
		if (!measuringOngoingGuard) return;

		// Wait for the changed content which fired the `afterUpdate()` Svelte event to be put into the DOM
		await tick();

		// Wait until all fonts have been loaded and rendered so measurements of content involving text are accurate
		await document.fonts.ready;

		// Make the component show itself with 0 min-width so it can be measured, and wait until the values have been updated to the DOM
		measuringOngoing = true;
		measuringOngoingGuard = true;
		await tick();

		// Measure the width of the floating menu content element, if it's currently visible
		// The result will be `undefined` if the menu is invisible, perhaps because an ancestor component is hidden with a falsy Svelte template if condition
		const naturalWidth: number | undefined = floatingMenuContent?.div().clientWidth;

		// Turn off measuring mode for the component, which triggers another call to the `afterUpdate()` Svelte event, so we can turn off the protection after that has happened
		measuringOngoing = false;
		await tick();
		measuringOngoingGuard = false;

		// Notify the parent about the measured natural width
		if (naturalWidth !== undefined && naturalWidth >= 0) {
			dispatch("naturalWidth", naturalWidth);
		}
	}

	function pointerMoveHandler(e: PointerEvent) {
		// This element and the element being hovered over
		const target = e.target as HTMLElement | undefined;

		// Get the spawner element (that which is clicked to spawn this floating menu)
		// Assumes the spawner is a sibling of this FloatingMenu component
		const ownSpawner: HTMLElement | undefined = self?.parentElement?.querySelector(":scope > [data-floating-menu-spawner]") || undefined;
		// Get the spawner element containing whatever element the user is hovering over now, if there is one
		const targetSpawner: HTMLElement | undefined = target?.closest("[data-floating-menu-spawner]") || undefined;

		// HOVER TRANSFER
		// Transfer from this open floating menu to a sibling floating menu if the pointer hovers to a valid neighboring floating menu spawner
		hoverTransfer(self, ownSpawner, targetSpawner);

		// POINTER STRAY
		// Close the floating menu if the pointer has strayed far enough from its bounds (and it's not hovering over its own spawner)
		const notHoveringOverOwnSpawner = ownSpawner !== targetSpawner;
		if (strayCloses && notHoveringOverOwnSpawner && isPointerEventOutsideFloatingMenu(e, POINTER_STRAY_DISTANCE)) {
			// TODO: Extend this rectangle bounds check to all submenu bounds up the DOM tree since currently submenus disappear
			// TODO: with zero stray distance if the cursor is further than the stray distance from only the top-level menu
			dispatch("open", false);
		}

		// Clean up any messes from lost pointerup events
		const eventIncludesLmb = Boolean(e.buttons & 1);
		if (!open && !eventIncludesLmb) {
			pointerStillDown = false;
			window.removeEventListener("pointerup", pointerUpHandler);
		}
	}

	function hoverTransfer(self: HTMLDivElement | undefined, ownSpawner: HTMLElement | undefined, targetSpawner: HTMLElement | undefined): void {
		// Algorithm pseudo-code to detect and transfer to hover-transferrable floating menu spawners
		// Accompanying diagram: <https://files.keavon.com/-/SpringgreenKnownXantus/capture.png>
		//
		// Check our own parent for descendant spawners
		// Filter out ourself and our children
		// Filter out all with a different distance than our own distance from the currently-being-checked parent
		// How many left?
		//     None -> go up a level and repeat
		//     Some -> is one of them the target?
		//         Yes -> click it and terminate
		//         No -> do nothing and terminate

		// Helper function that gets used below
		const getDepthFromAncestor = (item: Element, ancestor: Element): number | undefined => {
			let depth = 1;

			let parent = item.parentElement || undefined;
			while (parent) {
				if (parent === ancestor) return depth;

				parent = parent.parentElement || undefined;
				depth += 1;
			}

			return undefined;
		};

		// A list of all the descendant spawners: the spawner for this floating menu plus any spawners belonging to widgets inside this floating menu
		const ownDescendantMenuSpawners = Array.from(self?.parentElement?.querySelectorAll("[data-floating-menu-spawner]") || []);

		// Start with the parent of the spawner for this floating menu and keep widening the search for any other valid spawners that are hover-transferrable
		let currentAncestor = (targetSpawner && ownSpawner?.parentElement) || undefined;
		while (currentAncestor) {
			const ownSpawnerDepthFromCurrentAncestor = ownSpawner && getDepthFromAncestor(ownSpawner, currentAncestor);
			const currentAncestor2 = currentAncestor; // This duplicate variable avoids an ESLint warning

			// Get the list of descendant spawners and filter out invalid possibilities for spawners that are hover-transferrable
			const listOfDescendantSpawners = Array.from(currentAncestor?.querySelectorAll("[data-floating-menu-spawner]") || []);
			const filteredListOfDescendantSpawners = listOfDescendantSpawners.filter((item: Element): boolean => {
				// Filter away ourself and our descendants
				const notOurself = !ownDescendantMenuSpawners.includes(item);
				// And filter away unequal depths from the current ancestor
				const notUnequalDepths = notOurself && getDepthFromAncestor(item, currentAncestor2) === ownSpawnerDepthFromCurrentAncestor;
				// And filter away elements that explicitly disable hover transfer
				return notUnequalDepths && !(item as HTMLElement).getAttribute?.("data-floating-menu-spawner")?.includes("no-hover-transfer");
			});

			// If none were found, widen the search by a level and keep trying (or stop looping if the root was reached)
			if (filteredListOfDescendantSpawners.length === 0) {
				currentAncestor = currentAncestor?.parentElement || undefined;
			}
			// Stop after the first non-empty set was found
			else {
				const foundTarget = filteredListOfDescendantSpawners.find((item: Element): boolean => item === targetSpawner);
				// If the currently hovered spawner is one of the found valid hover-transferrable spawners, swap to it by clicking on it
				if (foundTarget) {
					dispatch("open", false);
					(foundTarget as HTMLElement).click();
				}

				// In either case, we are done searching
				break;
			}
		}
	}

	function keyDownHandler(e: KeyboardEvent) {
		if (escapeCloses && e.key.toLowerCase() === "escape") {
			dispatch("open", false);
		}
	}

	function pointerDownHandler(e: PointerEvent) {
		// Close the floating menu if the pointer clicked outside the floating menu (but within stray distance)
		if (isPointerEventOutsideFloatingMenu(e)) {
			dispatch("open", false);

			// Track if the left pointer button is now down so its later click event can be canceled
			const eventIsForLmb = e.button === 0;
			if (eventIsForLmb) pointerStillDown = true;
		}
	}

	function pointerUpHandler(e: PointerEvent) {
		const eventIsForLmb = e.button === 0;
		if (pointerStillDown && eventIsForLmb) {
			// Clean up self
			pointerStillDown = false;
			window.removeEventListener("pointerup", pointerUpHandler);
			// Prevent the click event from firing, which would normally occur right after this pointerup event
			window.addEventListener("click", clickHandlerCapture, true);
		}
	}

	function clickHandlerCapture(e: MouseEvent) {
		// Stop the click event from reopening this floating menu if the click event targets the floating menu's button
		e.stopPropagation();
		// Clean up self
		window.removeEventListener("click", clickHandlerCapture, true);
	}

	function isPointerEventOutsideFloatingMenu(e: PointerEvent, extraDistanceAllowed = 0): boolean {
		// Consider all child menus as well as the top-level one
		const allContainedFloatingMenus = [...self.querySelectorAll("[data-floating-menu-content]")];

		return !allContainedFloatingMenus.find((element) => !isPointerEventOutsideMenuElement(e, element, extraDistanceAllowed));
	}

	function isPointerEventOutsideMenuElement(e: PointerEvent, element: Element, extraDistanceAllowed = 0): boolean {
		const floatingMenuBounds = element.getBoundingClientRect();

		if (floatingMenuBounds.left - e.clientX >= extraDistanceAllowed) return true;
		if (e.clientX - floatingMenuBounds.right >= extraDistanceAllowed) return true;
		if (floatingMenuBounds.top - e.clientY >= extraDistanceAllowed) return true;
		if (e.clientY - floatingMenuBounds.bottom >= extraDistanceAllowed) return true;

		return false;
	}
</script>

<div
	class={`floating-menu ${direction.toLowerCase()} ${type.toLowerCase()} ${className} ${extraClasses}`.trim()}
	style={`${styleName} ${extraStyles}`.trim() || undefined}
	bind:this={self}
	{...$$restProps}
>
	{#if displayTail}
		<div class="tail" bind:this={tail} />
	{/if}
	{#if displayContainer}
		<div class="floating-menu-container" bind:this={floatingMenuContainer}>
			<LayoutCol class="floating-menu-content" styles={{ "min-width": minWidthStyleValue }} {scrollableY} bind:this={floatingMenuContent} data-floating-menu-content>
				<slot />
			</LayoutCol>
		</div>
	{/if}
</div>

<style lang="scss" global>
	.floating-menu {
		position: absolute;
		width: 0;
		height: 0;
		display: flex;
		// Floating menus begin at a z-index of 1000
		z-index: 1000;
		--floating-menu-content-offset: 0;
		--floating-menu-content-border-radius: 4px;

		&.bottom {
			--floating-menu-content-border-radius: 0 0 4px 4px;
		}

		.tail {
			width: 0;
			height: 0;
			border-style: solid;
			// Put the tail above the floating menu's shadow
			z-index: 10;
			// Draw over the application without being clipped by the containing panel's `overflow: hidden`
			position: fixed;
		}

		.floating-menu-container {
			display: flex;

			.floating-menu-content {
				background: rgba(var(--color-2-mildblack-rgb), 0.95);
				box-shadow: rgba(var(--color-0-black-rgb), 50%) 0 2px 4px;
				border-radius: var(--floating-menu-content-border-radius);
				color: var(--color-e-nearwhite);
				font-size: inherit;
				padding: 8px;
				z-index: 0;
				// Draw over the application without being clipped by the containing panel's `overflow: hidden`
				position: fixed;
			}
		}

		&.dropdown {
			&.top {
				width: 100%;
				left: 0;
				top: 0;
			}

			&.bottom {
				width: 100%;
				left: 0;
				bottom: 0;
			}

			&.left {
				height: 100%;
				top: 0;
				left: 0;
			}

			&.right {
				height: 100%;
				top: 0;
				right: 0;
			}

			&.topleft {
				top: 0;
				left: 0;
				margin-top: -4px;
			}

			&.topright {
				top: 0;
				right: 0;
				margin-top: -4px;
			}

			&.topleft {
				bottom: 0;
				left: 0;
				margin-bottom: -4px;
			}

			&.topright {
				bottom: 0;
				right: 0;
				margin-bottom: -4px;
			}
		}

		&.top.dropdown .floating-menu-container,
		&.bottom.dropdown .floating-menu-container {
			justify-content: left;
		}

		&.popover {
			--floating-menu-content-offset: 10px;
			--floating-menu-content-border-radius: 4px;
		}

		&.cursor .floating-menu-container .floating-menu-content {
			background: none;
			box-shadow: none;
			border-radius: 0;
			padding: 0;
		}

		&.center {
			justify-content: center;
			align-items: center;

			> .floating-menu-container > .floating-menu-content {
				transform: translate(-50%, -50%);
			}
		}

		&.top,
		&.bottom {
			flex-direction: column;
		}

		&.top .tail {
			border-width: 8px 6px 0 6px;
			border-color: rgba(var(--color-2-mildblack-rgb), 0.95) transparent transparent transparent;
			margin-left: -6px;
			margin-bottom: 2px;
		}

		&.bottom .tail {
			border-width: 0 6px 8px 6px;
			border-color: transparent transparent rgba(var(--color-2-mildblack-rgb), 0.95) transparent;
			margin-left: -6px;
			margin-top: 2px;
		}

		&.left .tail {
			border-width: 6px 0 6px 8px;
			border-color: transparent transparent transparent rgba(var(--color-2-mildblack-rgb), 0.95);
			margin-top: -6px;
			margin-right: 2px;
		}

		&.right .tail {
			border-width: 6px 8px 6px 0;
			border-color: transparent rgba(var(--color-2-mildblack-rgb), 0.95) transparent transparent;
			margin-top: -6px;
			margin-left: 2px;
		}

		&.top .floating-menu-container {
			justify-content: center;
			margin-bottom: var(--floating-menu-content-offset);
		}

		&.bottom .floating-menu-container {
			justify-content: center;
			margin-top: var(--floating-menu-content-offset);
		}

		&.left .floating-menu-container {
			align-items: center;
			margin-right: var(--floating-menu-content-offset);
		}

		&.right .floating-menu-container {
			align-items: center;
			margin-left: var(--floating-menu-content-offset);
		}
	}
</style>
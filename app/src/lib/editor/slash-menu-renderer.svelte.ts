import { getSlashMenuItems } from "./menu-items";
import type { SlashMenuItem } from "./slash-command";
import SlashMenu from "$lib/components/SlashMenu.svelte";
import { createSuggestionRenderer } from "./suggestion-renderer.svelte";

const renderSlashMenu = createSuggestionRenderer<SlashMenuItem>({
  component: SlashMenu,
  getItems: (query) => getSlashMenuItems({ query }),
});

export default renderSlashMenu;

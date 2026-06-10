import { getEmojiItems, type EmojiItem } from "./emoji-command";
import EmojiMenu from "$lib/components/EmojiMenu.svelte";
import { createSuggestionRenderer } from "./suggestion-renderer.svelte";

const renderEmojiMenu = createSuggestionRenderer<EmojiItem>({
  component: EmojiMenu,
  getItems: (query) => getEmojiItems({ query }),
});

export default renderEmojiMenu;

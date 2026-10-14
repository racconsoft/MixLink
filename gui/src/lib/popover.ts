import { writable } from "svelte/store";

/**
 * Id of the single open popover (a channel menu or an app picker), if any.
 * Ids look like `menu:<channelId>` or `apps:<channelId>`. Setting this to a new id
 * closes whichever popover was open before, since every popover's `open` state is
 * derived from `$openPopover === <its own id>`.
 */
export const openPopover = writable<string | null>(null);

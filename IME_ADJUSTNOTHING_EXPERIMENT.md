# Follow-up: Android IME window policy

After the Compose typing/IME jank fixes (draft isolation, single composer insets, frozen bubble gradients), some keyboard open/close stutter can remain.

## Why

With `windowSoftInputMode="adjustResize"` and edge-to-edge, Android animates the **window height** as the IME appears. Compose also applies IME padding on the composer. The chat `LazyColumn` height changes on every frame of that animation, so the list remeasures/relayouts even when keystroke-driven recomposition is gone. Large chats pay more because more rows participate in layout.

That cost is largely **system/window policy**, not app draft state.

## Suggested experiment

If IME open/close still feels heavy after the current fixes:

1. Change the activity to `adjustNothing` (or `adjustPan`) in `AndroidManifest.xml`.
2. Rely on **Compose-only** IME padding (already on the message composer via `WindowInsets.ime.union(WindowInsets.navigationBars)`).
3. Manually verify on a real device:
   - Keyboard does not cover the composer
   - Nav/gesture bar still clears
   - Unlock / settings fields still usable
   - No double gap under the keyboard

### Tradeoffs

- **Possible win:** Fixed window size → less list height thrash during IME animation.
- **Possible loss:** OEM differences; edge-to-edge overlap; need careful inset handling on all text fields (not only the chat composer).

Do not switch without measuring on device; keep `adjustResize` until then.

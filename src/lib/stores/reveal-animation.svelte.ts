/**
 * Global reveal-animation store — shared between Settings (writes) and chat page (reads).
 * Controls the animation style used when the drip-to-timeline handoff completes.
 */

export type RevealStyle =
  | "none"
  | "fade"
  | "slide"
  | "blur"
  | "scale"
  | "wipe"
  | "fly-in"
  | "decode"
  | "cascade";

export const REVEAL_STYLES: { value: RevealStyle; label: string; experimental?: boolean }[] = [
  { value: "none", label: "Standard" },
  { value: "fade", label: "Fade Edge" },
  { value: "blur", label: "Blur Edge" },
  { value: "decode", label: "Decode" },
  { value: "slide", label: "Slide Up", experimental: true },
  { value: "scale", label: "Scale Settle", experimental: true },
  { value: "wipe", label: "Wipe Down", experimental: true },
  { value: "fly-in", label: "Random Fly-In", experimental: true },
  { value: "cascade", label: "Signal Cascade", experimental: true },
];

let _style = $state<RevealStyle>("decode");

export const revealAnimationStore = {
  get value(): RevealStyle {
    return _style;
  },
  set value(v: RevealStyle) {
    _style = v;
  },
};

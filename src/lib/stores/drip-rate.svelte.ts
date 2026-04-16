/**
 * Global drip rate store — shared between Settings (writes) and chat page (reads).
 * Initialized from UserSettings on app load, updated immediately when the slider moves.
 */
let _dripRate = $state(35);

export const dripRateStore = {
  get value() {
    return _dripRate;
  },
  set value(v: number) {
    _dripRate = v;
  },
};

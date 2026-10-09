// Which parts of the app are switched on (commands/features.rs). Loaded by the layout
// before any page renders; Settings changes it. Core modules default on; the watch sync,
// the Records vault and each AI feature default off for a new user.

import { invoke } from '@tauri-apps/api/core';

export interface Features {
  onboarded: boolean;
  sleep: boolean;
  activity: boolean;
  cardio: boolean;
  medication: boolean;
  food: boolean;
  work: boolean;
  pacing: boolean;
  health_sync: boolean;
  vault: boolean;
  ai: boolean;
  ai_ask: boolean;
  ai_weekly: boolean;
  ai_food_photo: boolean;
  ai_food_tags: boolean;
  ai_records: boolean;
}

export type AiFeature = 'ai_ask' | 'ai_weekly' | 'ai_food_photo' | 'ai_food_tags' | 'ai_records';

export const features = $state<Features>({
  onboarded: true,
  sleep: true, activity: true, cardio: true, medication: true, food: true, work: true, pacing: true,
  health_sync: false, vault: false,
  ai: false, ai_ask: false, ai_weekly: false, ai_food_photo: false, ai_food_tags: false, ai_records: false,
});

export async function loadFeatures() {
  Object.assign(features, await invoke<Features>('get_features'));
}

/** Change some switches and save them. */
export async function setFeatures(patch: Partial<Features>) {
  Object.assign(features, patch);
  await invoke('save_features', { features: { ...features } });
}

/** An AI feature is on only when AI itself is on too. */
export function aiOn(f: AiFeature): boolean {
  return features.ai && features[f];
}

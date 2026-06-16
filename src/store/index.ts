import { create } from "zustand";
import type { MediaInfo, ConversionParams } from "../types";
import type { TransitionStyle } from "../transitions";
import { DEFAULT_TRANSITION } from "../transitions";

interface GaldrState {
  mediaInfo: MediaInfo | null;
  conversionParams: ConversionParams;
  isConverting: boolean;
  conversionProgress: number;
  lastOutputPath: string | null;
  error: string | null;
  ffmpegFound: boolean;
  outputDir: string;
  transitionStyle: TransitionStyle;
  testTransitionSignal: number;

  setMediaInfo: (info: MediaInfo | null) => void;
  setConversionParams: (params: Partial<ConversionParams>) => void;
  setIsConverting: (v: boolean) => void;
  setConversionProgress: (v: number) => void;
  setLastOutputPath: (v: string | null) => void;
  setError: (v: string | null) => void;
  setFfmpegFound: (v: boolean) => void;
  setOutputDir: (v: string) => void;
  setTransitionStyle: (v: TransitionStyle) => void;
  triggerTransitionTest: () => void;
  reset: () => void;
}

const defaultParams: ConversionParams = {
  input_path: "",
  output_dir: "",
  output_format: "mp4",
  video_codec: undefined,
  audio_codec: undefined,
  video_bitrate: undefined,
  audio_bitrate: undefined,
  resolution: undefined,
  framerate: undefined,
  crf: undefined,
  preset: undefined,
};

export const useGaldrStore = create<GaldrState>((set) => ({
  mediaInfo: null,
  conversionParams: { ...defaultParams },
  isConverting: false,
  conversionProgress: 0,
  lastOutputPath: null,
  error: null,
  ffmpegFound: false,
  outputDir: "",
  transitionStyle: DEFAULT_TRANSITION,
  testTransitionSignal: 0,

  setMediaInfo: (info) => set({ mediaInfo: info }),
  setConversionParams: (params) =>
    set((state) => ({
      conversionParams: { ...state.conversionParams, ...params },
    })),
  setIsConverting: (v) => set({ isConverting: v }),
  setConversionProgress: (v) => set({ conversionProgress: v }),
  setLastOutputPath: (v) => set({ lastOutputPath: v }),
  setError: (v) => set({ error: v }),
  setFfmpegFound: (v) => set({ ffmpegFound: v }),
  setOutputDir: (v) => set({ outputDir: v }),
  setTransitionStyle: (v) => set({ transitionStyle: v }),
  triggerTransitionTest: () => set((s) => ({ testTransitionSignal: s.testTransitionSignal + 1 })),
  reset: () =>
    set({
      mediaInfo: null,
      conversionParams: { ...defaultParams },
      isConverting: false,
      conversionProgress: 0,
      lastOutputPath: null,
      error: null,
    }),
}));

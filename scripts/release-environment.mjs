// Compatibility export for callers that still import the former module name.
// All release/distribution policy now lives in release-channel.mjs.
export {
  releaseChannelEnvironment,
  resolveBuildProfile,
  resolveDistribution,
  resolveReleaseChannel,
  resolveTauriInvocation,
  tauriEnvironment,
} from "./release-channel.mjs";

//! Built-in provider catalog for the desktop client.
//!
//! The sidecar ships pi's full provider registry, but the desktop has to stay
//! usable while the sidecar is starting (or after the user closes it), and the
//! provider picker must not fall back to a three-item list. This module is the
//! single source of truth for:
//!
//! * the provider picker / "add provider" experience (name, endpoint, auth),
//! * the native Rust engine's endpoint + API-key fallbacks,
//! * a small curated model seed per provider, used only when a credential is
//!   configured but no model list could be fetched from the sidecar/upstream.
//!
//! The seed lists are deliberately short and only act as a safety net: model
//! discovery (`GET {base}/models`) is always preferred, and pi's bundled model
//! catalog wins whenever the sidecar is reachable.

/// How popular/where a provider comes from; used to group the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCategory {
    /// First-party model vendors (OpenAI, Anthropic, ...).
    Global,
    /// Multi-model gateways and aggregators.
    Gateway,
    /// Providers whose primary audience is mainland China.
    China,
    /// Local inference servers (no cloud credential).
    Local,
    /// Subscription/OAuth-first providers.
    Subscription,
}

impl ProviderCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Gateway => "Gateways",
            Self::China => "China",
            Self::Local => "Local",
            Self::Subscription => "Subscriptions",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Global => "国际供应商",
            Self::Gateway => "聚合网关",
            Self::China => "中国供应商",
            Self::Local => "本地推理",
            Self::Subscription => "订阅登录",
        }
    }
}

/// A curated model that is offered when no live model list is available.
#[derive(Debug, Clone, Copy)]
pub struct SeedModel {
    pub id: &'static str,
    pub name: &'static str,
    pub reasoning: bool,
    pub context_window: u64,
}

const fn seed(
    id: &'static str,
    name: &'static str,
    reasoning: bool,
    context_window: u64,
) -> SeedModel {
    SeedModel {
        id,
        name,
        reasoning,
        context_window,
    }
}

/// One provider the desktop knows how to talk to.
#[derive(Debug, Clone, Copy)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    /// Wire protocol: `openai-completions`, `openai-responses`,
    /// `anthropic-messages`, or `google-generative-ai`.
    pub api: &'static str,
    pub env_vars: &'static [&'static str],
    pub api_key: bool,
    pub oauth: bool,
    /// Local servers accept requests without an API key.
    pub optional_api_key: bool,
    pub category: ProviderCategory,
    pub models: &'static [SeedModel],
}

const OPENAI_MODELS: &[SeedModel] = &[
    seed("gpt-5.2", "GPT-5.2", true, 400_000),
    seed("gpt-5.3-codex", "GPT-5.3 Codex", true, 400_000),
    seed("gpt-5-mini", "GPT-5 Mini", true, 400_000),
];

const ANTHROPIC_MODELS: &[SeedModel] = &[
    seed("claude-fable-5-2", "Claude Fable 5.2", true, 1_000_000),
    seed("claude-opus-4-5", "Claude Opus 4.5", true, 200_000),
    seed("claude-haiku-4-5", "Claude Haiku 4.5", true, 200_000),
];

const GOOGLE_MODELS: &[SeedModel] = &[
    seed("gemini-3.5-flash", "Gemini 3.5 Flash", true, 1_048_576),
    seed("gemini-2.5-pro", "Gemini 2.5 Pro", true, 1_048_576),
    seed("gemini-3.1-flash-lite", "Gemini 3.1 Flash Lite", true, 1_048_576),
];

const XAI_MODELS: &[SeedModel] = &[
    seed("grok-4.7", "Grok 4.7", true, 500_000),
    seed("grok-4.5", "Grok 4.5", true, 500_000),
];

const DEEPSEEK_MODELS: &[SeedModel] = &[
    seed("deepseek-v4-pro", "DeepSeek V4 Pro", true, 1_000_000),
    seed("deepseek-flash", "DeepSeek V4.1 Flash", true, 1_000_000),
];

const MISTRAL_MODELS: &[SeedModel] = &[
    seed("devstral-latest", "Devstral 2", false, 262_144),
    seed("codestral-latest", "Codestral (latest)", false, 256_000),
];

const GROQ_MODELS: &[SeedModel] = &[
    seed("openai/gpt-oss-120b", "GPT OSS 120B", true, 131_072),
    seed("llama-3.3-70b-versatile", "Llama 3.3 70B", false, 131_072),
];

const TOGETHER_MODELS: &[SeedModel] = &[
    seed("moonshotai/Kimi-K3", "Kimi K3", true, 1_048_576),
    seed("deepseek-ai/DeepSeek-V4-Pro-0813", "DeepSeek V4 Pro", true, 1_048_576),
];

const FIREWORKS_MODELS: &[SeedModel] = &[
    seed("accounts/fireworks/models/deepseek-v4p1-flash", "DeepSeek V4.1 Flash", true, 1_000_000),
    seed("accounts/fireworks/models/minimax-m3", "MiniMax-M3", true, 512_000),
];

const CEREBRAS_MODELS: &[SeedModel] = &[
    seed("gpt-oss-120b", "GPT OSS 120B", true, 131_072),
    seed("qwen-3.8-27b", "Qwen3.8 27B", true, 131_072),
];

const NVIDIA_MODELS: &[SeedModel] = &[
    seed("deepseek-ai/deepseek-v4.1-flash", "DeepSeek V4.1 Flash", true, 1_000_000),
    seed("meta/llama-3.2-90b-vision-instruct", "Llama 3.2 90B Vision", false, 128_000),
];

const HUGGINGFACE_MODELS: &[SeedModel] = &[
    seed("deepseek-ai/DeepSeek-V3.1", "DeepSeek V3.1", true, 131_072),
    seed("meta-llama/Llama-3.3-70B-Instruct", "Llama 3.3 70B Instruct", false, 131_072),
];

const BASETEN_MODELS: &[SeedModel] = &[
    seed("deepseek-ai/DeepSeek-V4-Pro-0813", "DeepSeek V4 Pro", true, 1_048_576),
    seed("moonshotai/Kimi-K3", "Kimi K3", true, 1_048_576),
];

const ANT_LING_MODELS: &[SeedModel] = &[
    seed("Ling-2.6-1T", "Ling 2.6 1T", true, 262_144),
    seed("Ling-2.6-flash", "Ling 2.6 Flash", true, 262_144),
];

const OPENROUTER_MODELS: &[SeedModel] = &[
    seed("anthropic/claude-fable-5", "Anthropic: Claude Fable 5", true, 1_000_000),
    seed("openai/gpt-5.2", "OpenAI: GPT-5.2", true, 400_000),
    seed("google/gemini-3.5-flash", "Google: Gemini 3.5 Flash", true, 1_048_576),
    seed("deepseek/deepseek-v4-pro", "DeepSeek: V4 Pro", true, 1_000_000),
    seed("moonshotai/kimi-k3.1-pro", "MoonshotAI: Kimi K3.1 Pro", true, 1_048_576),
    seed("z-ai/glm-5.3", "Z.AI: GLM-5.3", true, 1_000_000),
];

const VERCEL_MODELS: &[SeedModel] = &[
    seed("alibaba/qwen3-coder", "Qwen3 Coder 480B", false, 262_144),
    seed("anthropic/claude-fable-5", "Anthropic: Claude Fable 5", true, 1_000_000),
];

const SILICONFLOW_MODELS: &[SeedModel] = &[
    seed("deepseek-ai/DeepSeek-V3.2", "DeepSeek V3.2", true, 163_840),
    seed("Qwen/Qwen3-235B-A22B-Instruct-2507", "Qwen3 235B", true, 262_144),
];

const MOONSHOT_MODELS: &[SeedModel] = &[
    seed("kimi-k3.1-pro", "Kimi K3.1 Pro", true, 1_048_576),
    seed("kimi-k2.7-code", "Kimi K2.7 Code", true, 262_144),
];

const MINIMAX_MODELS: &[SeedModel] = &[
    seed("MiniMax-M3", "MiniMax-M3", true, 1_000_000),
    seed("MiniMax-M2.7", "MiniMax-M2.7", true, 204_800),
];

const ZAI_MODELS: &[SeedModel] = &[
    seed("glm-5.3", "GLM-5.3", true, 1_000_000),
    seed("glm-4.7", "GLM-4.7", true, 204_800),
];

const QWEN_MODELS: &[SeedModel] = &[
    seed("qwen3-max", "Qwen3 Max", true, 262_144),
    seed("qwen3-coder-plus", "Qwen3 Coder Plus", false, 1_000_000),
];

const XIAOMI_MODELS: &[SeedModel] = &[seed("mimo-v2", "MiMo V2", true, 262_144)];

const KIMI_CODING_MODELS: &[SeedModel] = &[
    seed("kimi-k2.7-code", "Kimi K2.7 Code", true, 262_144),
    seed("kimi-k3", "Kimi K3", true, 1_048_576),
];

const COPILOT_MODELS: &[SeedModel] = &[
    seed("claude-fable-5", "Claude Fable 5", true, 1_000_000),
    seed("gpt-5.2", "GPT-5.2", true, 400_000),
];

const OLLAMA_MODELS: &[SeedModel] = &[
    seed("qwen3-coder:30b", "Qwen3 Coder 30B", false, 262_144),
    seed("gpt-oss:20b", "GPT OSS 20B", true, 131_072),
];

const LMSTUDIO_MODELS: &[SeedModel] = &[
    seed("qwen/qwen3-coder-30b", "Qwen3 Coder 30B", false, 262_144),
    seed("openai/gpt-oss-20b", "GPT OSS 20B", true, 131_072),
];

const VLLM_MODELS: &[SeedModel] = &[];

const VOLCENGINE_MODELS: &[SeedModel] = &[seed("doubao-seed-1.6", "Doubao Seed 1.6", true, 262_144)];

/// Every provider preset, in picker order. The first entry of each category is
/// the most common choice so the dialog opens on familiar names.
pub const PROVIDERS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "openai",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        api: "openai-responses",
        env_vars: &["OPENAI_API_KEY"],
        api_key: true,
        oauth: true,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: OPENAI_MODELS,
    },
    ProviderPreset {
        id: "anthropic",
        name: "Anthropic",
        base_url: "https://api.anthropic.com",
        api: "anthropic-messages",
        env_vars: &["ANTHROPIC_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: ANTHROPIC_MODELS,
    },
    ProviderPreset {
        id: "google",
        name: "Google Gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        api: "google-generative-ai",
        env_vars: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: GOOGLE_MODELS,
    },
    ProviderPreset {
        id: "xai",
        name: "xAI (Grok)",
        base_url: "https://api.x.ai/v1",
        api: "openai-responses",
        env_vars: &["XAI_API_KEY"],
        api_key: true,
        oauth: true,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: XAI_MODELS,
    },
    ProviderPreset {
        id: "deepseek",
        name: "DeepSeek",
        base_url: "https://api.deepseek.com",
        api: "openai-completions",
        env_vars: &["DEEPSEEK_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: DEEPSEEK_MODELS,
    },
    ProviderPreset {
        id: "mistral",
        name: "Mistral AI",
        base_url: "https://api.mistral.ai/v1",
        api: "openai-completions",
        env_vars: &["MISTRAL_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: MISTRAL_MODELS,
    },
    ProviderPreset {
        id: "groq",
        name: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        api: "openai-completions",
        env_vars: &["GROQ_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: GROQ_MODELS,
    },
    ProviderPreset {
        id: "together",
        name: "Together AI",
        base_url: "https://api.together.ai/v1",
        api: "openai-completions",
        env_vars: &["TOGETHER_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: TOGETHER_MODELS,
    },
    ProviderPreset {
        id: "fireworks",
        name: "Fireworks AI",
        base_url: "https://api.fireworks.ai/inference/v1",
        api: "openai-completions",
        env_vars: &["FIREWORKS_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: FIREWORKS_MODELS,
    },
    ProviderPreset {
        id: "cerebras",
        name: "Cerebras",
        base_url: "https://api.cerebras.ai/v1",
        api: "openai-completions",
        env_vars: &["CEREBRAS_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: CEREBRAS_MODELS,
    },
    ProviderPreset {
        id: "nvidia",
        name: "NVIDIA NIM",
        base_url: "https://integrate.api.nvidia.com/v1",
        api: "openai-completions",
        env_vars: &["NVIDIA_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: NVIDIA_MODELS,
    },
    ProviderPreset {
        id: "huggingface",
        name: "Hugging Face",
        base_url: "https://router.huggingface.co/v1",
        api: "openai-completions",
        env_vars: &["HF_TOKEN"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: HUGGINGFACE_MODELS,
    },
    ProviderPreset {
        id: "baseten",
        name: "Baseten",
        base_url: "https://inference.baseten.co/v1",
        api: "openai-completions",
        env_vars: &["BASETEN_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: BASETEN_MODELS,
    },
    ProviderPreset {
        id: "ant-ling",
        name: "Ant Ling",
        base_url: "https://api.ant-ling.com/v1",
        api: "openai-completions",
        env_vars: &["ANT_LING_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Global,
        models: ANT_LING_MODELS,
    },
    ProviderPreset {
        id: "openrouter",
        name: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        api: "openai-completions",
        env_vars: &["OPENROUTER_API_KEY"],
        api_key: true,
        oauth: true,
        optional_api_key: false,
        category: ProviderCategory::Gateway,
        models: OPENROUTER_MODELS,
    },
    ProviderPreset {
        id: "vercel-ai-gateway",
        name: "Vercel AI Gateway",
        base_url: "https://ai-gateway.vercel.sh/v1",
        api: "openai-completions",
        env_vars: &["AI_GATEWAY_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Gateway,
        models: VERCEL_MODELS,
    },
    ProviderPreset {
        id: "siliconflow",
        name: "SiliconFlow",
        base_url: "https://api.siliconflow.cn/v1",
        api: "openai-completions",
        env_vars: &["SILICONFLOW_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Gateway,
        models: SILICONFLOW_MODELS,
    },
    ProviderPreset {
        id: "volcengine",
        name: "Volcengine Ark (Doubao)",
        base_url: "https://ark.cn-beijing.volces.com/api/v3",
        api: "openai-completions",
        env_vars: &["ARK_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::Gateway,
        models: VOLCENGINE_MODELS,
    },
    ProviderPreset {
        id: "moonshotai",
        name: "Moonshot AI (Kimi)",
        base_url: "https://api.moonshot.ai/v1",
        api: "openai-completions",
        env_vars: &["MOONSHOT_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: MOONSHOT_MODELS,
    },
    ProviderPreset {
        id: "moonshotai-cn",
        name: "Moonshot AI CN",
        base_url: "https://api.moonshot.cn/v1",
        api: "openai-completions",
        env_vars: &["MOONSHOT_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: MOONSHOT_MODELS,
    },
    ProviderPreset {
        id: "minimax",
        name: "MiniMax",
        base_url: "https://api.minimax.io/v1",
        api: "openai-completions",
        env_vars: &["MINIMAX_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: MINIMAX_MODELS,
    },
    ProviderPreset {
        id: "minimax-cn",
        name: "MiniMax CN",
        base_url: "https://api.minimaxi.com/v1",
        api: "openai-completions",
        env_vars: &["MINIMAX_CN_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: MINIMAX_MODELS,
    },
    ProviderPreset {
        id: "zai",
        name: "Z.AI (GLM)",
        base_url: "https://api.z.ai/api/coding/paas/v4",
        api: "openai-completions",
        env_vars: &["ZAI_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: ZAI_MODELS,
    },
    ProviderPreset {
        id: "zai-coding-cn",
        name: "Z.AI Coding CN",
        base_url: "https://open.bigmodel.cn/api/coding/paas/v4",
        api: "openai-completions",
        env_vars: &["ZAI_CODING_CN_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: ZAI_MODELS,
    },
    ProviderPreset {
        id: "qwen",
        name: "Qwen (DashScope Intl)",
        base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        api: "openai-completions",
        env_vars: &["DASHSCOPE_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: QWEN_MODELS,
    },
    ProviderPreset {
        id: "qwen-cn",
        name: "Qwen (DashScope CN)",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        api: "openai-completions",
        env_vars: &["DASHSCOPE_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: QWEN_MODELS,
    },
    ProviderPreset {
        id: "xiaomi",
        name: "Xiaomi MiMo",
        base_url: "https://api.xiaomimimo.com/v1",
        api: "openai-completions",
        env_vars: &["XIAOMI_API_KEY"],
        api_key: true,
        oauth: false,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: XIAOMI_MODELS,
    },
    ProviderPreset {
        id: "kimi-coding",
        name: "Kimi For Coding",
        base_url: "https://api.kimi.com/coding/v1",
        api: "openai-completions",
        env_vars: &["KIMI_API_KEY"],
        api_key: true,
        oauth: true,
        optional_api_key: false,
        category: ProviderCategory::China,
        models: KIMI_CODING_MODELS,
    },
    ProviderPreset {
        id: "github-copilot",
        name: "GitHub Copilot",
        base_url: "https://api.individual.githubcopilot.com",
        api: "openai-completions",
        env_vars: &[],
        api_key: false,
        oauth: true,
        optional_api_key: false,
        category: ProviderCategory::Subscription,
        models: COPILOT_MODELS,
    },
    ProviderPreset {
        id: "ollama",
        name: "Ollama (local)",
        base_url: "http://localhost:11434/v1",
        api: "openai-completions",
        env_vars: &[],
        api_key: false,
        oauth: false,
        optional_api_key: true,
        category: ProviderCategory::Local,
        models: OLLAMA_MODELS,
    },
    ProviderPreset {
        id: "lmstudio",
        name: "LM Studio (local)",
        base_url: "http://localhost:1234/v1",
        api: "openai-completions",
        env_vars: &[],
        api_key: false,
        oauth: false,
        optional_api_key: true,
        category: ProviderCategory::Local,
        models: LMSTUDIO_MODELS,
    },
    ProviderPreset {
        id: "vllm",
        name: "vLLM (local)",
        base_url: "http://localhost:8000/v1",
        api: "openai-completions",
        env_vars: &[],
        api_key: false,
        oauth: false,
        optional_api_key: true,
        category: ProviderCategory::Local,
        models: VLLM_MODELS,
    },
];

/// Provider ids that pi's own runtime already knows about. Everything else in
/// this catalog has to be registered in `models.json` so the sidecar and the
/// native engine can both use it.
const PI_BUILTIN_IDS: &[&str] = &[
    "amazon-bedrock",
    "ant-ling",
    "anthropic",
    "azure",
    "baseten",
    "cerebras",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "deepseek",
    "fireworks",
    "github-copilot",
    "google",
    "google-vertex",
    "groq",
    "huggingface",
    "kimi-coding",
    "meta",
    "minimax",
    "minimax-cn",
    "mistral",
    "moonshotai",
    "moonshotai-cn",
    "nvidia",
    "openai",
    "openai-codex",
    "opencode",
    "opencode-go",
    "openrouter",
    "qwen-token-plan",
    "qwen-token-plan-cn",
    "qwen-token-plan-individual",
    "qwen-token-plan-individual-cn",
    "radius",
    "together",
    "typesafe",
    "vercel-ai-gateway",
    "xai",
    "xiaomi",
    "xiaomi-token-plan-ams",
    "xiaomi-token-plan-cn",
    "xiaomi-token-plan-sgp",
    "zai",
    "zai-coding-cn",
];

/// Whether pi's runtime ships this provider out of the box.
pub fn is_pi_builtin(id: &str) -> bool {
    PI_BUILTIN_IDS.iter().any(|known| known.eq_ignore_ascii_case(id))
}

/// Every preset in picker order.
pub fn all() -> &'static [ProviderPreset] {
    PROVIDERS
}

/// Look up a preset by provider id (case-insensitive).
pub fn find(id: &str) -> Option<&'static ProviderPreset> {
    let needle = id.trim();
    PROVIDERS
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(needle))
}

pub fn is_builtin(id: &str) -> bool {
    find(id).is_some()
}

pub fn base_url_for(id: &str) -> Option<&'static str> {
    find(id).map(|p| p.base_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn provider_ids_are_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for p in PROVIDERS {
            assert!(seen.insert(p.id), "duplicate provider id: {}", p.id);
            assert!(!p.id.is_empty());
            assert!(!p.name.is_empty());
            assert!(
                p.base_url.starts_with("http://") || p.base_url.starts_with("https://"),
                "{} has an invalid base url: {}",
                p.id,
                p.base_url
            );
            assert!(
                matches!(
                    p.api,
                    "openai-completions"
                        | "openai-responses"
                        | "anthropic-messages"
                        | "google-generative-ai"
                ),
                "{} has an unsupported api: {}",
                p.id,
                p.api
            );
            assert!(
                p.api_key || p.oauth || p.optional_api_key,
                "{} has no auth method",
                p.id
            );
        }
    }

    #[test]
    fn popular_providers_are_present() {
        for id in [
            "openai",
            "anthropic",
            "google",
            "openrouter",
            "deepseek",
            "xai",
            "groq",
            "mistral",
            "moonshotai",
            "zai",
            "qwen",
            "ollama",
        ] {
            assert!(find(id).is_some(), "missing provider preset: {id}");
        }
    }

    #[test]
    fn seed_models_are_coherent() {
        for p in PROVIDERS {
            for m in p.models {
                assert!(!m.id.is_empty());
                assert!(!m.name.is_empty());
                assert!(m.context_window > 0, "{}:{} has no context window", p.id, m.id);
            }
        }
    }
}

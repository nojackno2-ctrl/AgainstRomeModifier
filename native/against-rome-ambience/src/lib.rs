//! Faithful ports of the managed editor ambience and localization modules.
pub mod app_localization;
pub mod atmosphere;
pub mod localization;
pub mod soundscape;

pub use app_localization::{Language, Loc};
pub use atmosphere::{
    AtmosphereEditSession, AtmosphereLightingBridge, AtmosphereProfile,
    AtmosphereShaderUniforms, BodenIniData, BodenIniSerializer, PrecipitationType,
    WeatherPresets,
};
pub use localization::{
    BriefingSaveTransaction, BriefingTextTable, BriefingTextTableCatalog,
    BriefingVariableCategory, BriefingVariableDescriptor, BriefingVariableType,
    MultiLanguageTextStudio, PutIssueSeverity, PutScriptDocument, PutScriptIssue,
    PutScriptNode, PutScriptSyntaxValidator, PutScriptValidationReport,
    SupportedLanguage, TranslationGlossary,
};
pub use soundscape::{
    AudibleChannelEvaluation, AudioGizmoRepresentation, AttenuationCurveKind, AudioCategory,
    SoundDef, SoundTriggerMode, SoundscapeBinaryStorage, SoundscapeEditorModule,
    SoundscapeError, SoundscapeMixEvaluator, SoundscapeMixResult, SoundscapeShapeType,
    SoundscapeSnapshot, SoundscapeVertex, SoundscapeZone, SoundscapeZoneAttributes,
    SoundscapeZoneCatalog, SoundscapeZoneFlags, SoundscapeZonePlanner,
};

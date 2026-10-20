use against_rome_campaign::{
    CampaignMissionCompiler, CampaignMissionPlan, CampaignObjective, CampaignObjectiveType,
    WaveAttackDefinition, WaveSquadDefinition,
};
use against_rome_scenario::ScenarioDocument;
use uuid::Uuid;

#[test]
fn wave_survival_delay_overflow_is_a_validation_failure_not_a_panic() {
    // C# CampaignMissionCompiler uses unchecked int addition, then rejects the
    // wrapped (negative) delay in ScenarioEventValidator. Keep the same result
    // in debug and release builds, including the 120-second default buffer.
    let aliases = vec!["legionary".to_string()];
    let scenario = ScenarioDocument::default();
    for (wave_delay, buffer, expected_delay) in [
        (1, 86_399, 86_400),
        (1, 86_400, 86_401),
        (1, i32::MAX, i32::MIN),
        (i32::MAX, 0, i32::MIN + 119),
    ] {
        let mut plan = CampaignMissionPlan::new("Overflow", "");
        plan.waves.push(WaveAttackDefinition::new(
            1,
            wave_delay,
            "",
            100.0,
            100.0,
            vec![WaveSquadDefinition::new("legionary", 1, 1)],
        ));
        plan.objectives.push(
            CampaignObjective::new(
                Uuid::from_u128(1),
                CampaignObjectiveType::WaveSurvival,
                "Repel",
                "",
            )
            .with_seconds(buffer),
        );
        assert!(plan.validate().is_ok());

        for result in [
            CampaignMissionCompiler::compile(&plan, None, &aliases, Some(&scenario)),
            CampaignMissionCompiler::compile_for_merge(&plan, &aliases, &scenario),
        ] {
            assert_eq!(result.success, expected_delay == 86_400);
            assert!(result.bci_bytes.is_none());
            assert_eq!(result.compiled_events[1].delay_seconds, expected_delay);
            if !result.success {
                assert!(result
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("Compiled ScenarioEvent validation failed")));
            }
        }
        assert!(scenario.events.is_empty());
    }
}

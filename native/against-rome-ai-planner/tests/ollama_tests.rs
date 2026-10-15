use std::sync::Arc;
use against_rome_ai_planner::ollama::{
    AiMaterialOption, HttpResponse, MockHttpTransport, OllamaMapPlanner,
};

#[test]
fn test_list_models_parses_and_sorts_alphabetically() {
    let mock = Arc::new(MockHttpTransport::new(|method, url, _body| {
        assert_eq!("GET", method);
        assert!(url.ends_with("/api/tags"));
        Ok(HttpResponse::new(
            200,
            r#"{"models":[{"name":"llama3"},{"name":"deepseek-r1"},{"name":"codellama"}]}"#,
        ))
    }));

    let planner = OllamaMapPlanner::new(Some("http://ollama.test"), Some(mock.clone()));
    let models = planner.list_models().unwrap();

    assert_eq!(vec!["codellama", "deepseek-r1", "llama3"], models);
    assert_eq!(1, mock.request_count());
}

#[test]
fn test_generate_plan_posts_schema_and_messages_and_parses_response() {
    let plan_json = r#"{"baseHeight":70,"baseMaterial":"grass","features":[{"type":"HILL","x":32,"y":32,"radius":6,"amount":50}]}"#;
    let plan_json_clone = plan_json.to_string();

    let mock = Arc::new(MockHttpTransport::new(move |method, url, body| {
        assert_eq!("POST", method);
        assert_eq!("http://ollama.test/api/chat", url);
        let body_str = body.unwrap();
        let doc: serde_json::Value = serde_json::from_str(body_str).unwrap();

        assert_eq!("test-model", doc["model"].as_str().unwrap());
        assert_eq!(false, doc["stream"].as_bool().unwrap());
        assert!(doc["format"].is_object());

        let messages = doc["messages"].as_array().unwrap();
        assert_eq!(2, messages.len());
        assert_eq!("system", messages[0]["role"].as_str().unwrap());
        assert_eq!("user", messages[1]["role"].as_str().unwrap());
        assert_eq!("A green valley", messages[1]["content"].as_str().unwrap());

        let resp_body = serde_json::json!({
            "message": {
                "content": plan_json_clone
            }
        });
        Ok(HttpResponse::new(200, resp_body.to_string()))
    }));

    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];

    let planner = OllamaMapPlanner::new(Some("http://ollama.test"), Some(mock.clone()));
    let (plan, raw) = planner
        .generate_plan("test-model", "A green valley", &materials, 60.0)
        .unwrap();

    assert_eq!(plan_json, raw);
    assert_eq!(Some(70), plan.base_height);
    assert_eq!(Some("Grass".to_string()), plan.base_material);
    assert_eq!(1, plan.features.len());
    let f = &plan.features[0];
    assert_eq!("hill", f.feature_type.as_deref().unwrap());
    assert_eq!((32, 32, 6, Some(50)), (f.x, f.y, f.radius, f.amount));
    assert_eq!(1, mock.request_count());
}

#[test]
fn test_list_models_returns_error_for_non_200() {
    let mock = Arc::new(MockHttpTransport::new(|_, _, _| {
        Ok(HttpResponse::new(503, "Service Unavailable"))
    }));
    let planner = OllamaMapPlanner::new(Some("http://ollama.test"), Some(mock));
    assert!(planner.list_models().is_err());
}

#[test]
fn test_generate_plan_returns_error_for_non_200() {
    let mock = Arc::new(MockHttpTransport::new(|_, _, _| {
        Ok(HttpResponse::new(400, "Bad Request"))
    }));
    let planner = OllamaMapPlanner::new(Some("http://ollama.test"), Some(mock));
    let materials = vec![AiMaterialOption::new("Grass", "草地")];
    assert!(planner
        .generate_plan("test-model", "test desc", &materials, 60.0)
        .is_err());
}

#[test]
fn test_generate_plan_rejects_blank_model_or_description_before_http() {
    let mock = Arc::new(MockHttpTransport::new(|_, _, _| {
        Ok(HttpResponse::new(200, "{}"))
    }));
    let planner = OllamaMapPlanner::new(Some("http://ollama.test"), Some(mock.clone()));
    let materials = vec![AiMaterialOption::new("Grass", "草地")];

    for blank in &["", " \t\n"] {
        assert!(planner
            .generate_plan(blank, "desc", &materials, 60.0)
            .is_err());
        assert!(planner
            .generate_plan("model", blank, &materials, 60.0)
            .is_err());
    }

    assert_eq!(0, mock.request_count());
}

#[test]
fn test_build_system_prompt_mentions_water_level_and_materials() {
    let materials = vec![
        AiMaterialOption::new("Grass", "草地"),
        AiMaterialOption::new("Sand", "沙地"),
    ];
    let prompt = OllamaMapPlanner::build_system_prompt(&materials, 73.0);
    assert!(prompt.contains("Ground below 73 becomes water"));
    assert!(prompt.contains("Grass: 草地"));
    assert!(prompt.contains("Sand: 沙地"));
}

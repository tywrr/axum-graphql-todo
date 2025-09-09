use axum::{
    Router,
    extract::{Extension, Json},
    response::Html,
    routing::{get, post},
};
use juniper::http::{GraphQLRequest, graphiql::graphiql_source};
use juniper::{EmptySubscription, FieldResult, GraphQLObject, RootNode, graphql_object};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, GraphQLObject)]
#[graphql(Context = Context)]
struct Todo {
    id: String,
    title: String,
    completed: bool,
}

#[derive(Clone)]
struct Context {
    store: Arc<Mutex<Vec<Todo>>>,
}
impl juniper::Context for Context {}

struct QueryRoot;
struct MutationRoot;

#[graphql_object(context = Context)]
impl QueryRoot {
    fn todos(context: &Context) -> Vec<Todo> {
        context.store.lock().clone()
    }

    fn todo(context: &Context, id: String) -> Option<Todo> {
        context.store.lock().iter().find(|t| t.id == id).cloned()
    }
}

#[graphql_object(context = Context)]
impl MutationRoot {
    fn create_todo(context: &Context, title: String) -> FieldResult<Todo> {
        let todo = Todo {
            id: Uuid::new_v4().to_string(),
            title,
            completed: false,
        };
        context.store.lock().push(todo.clone());
        Ok(todo)
    }

    fn toggle_todo(context: &Context, id: String) -> FieldResult<Option<Todo>> {
        let mut store = context.store.lock();
        if let Some(t) = store.iter_mut().find(|t| t.id == id) {
            t.completed = !t.completed;
            return Ok(Some(t.clone()));
        }
        Ok(None)
    }

    fn delete_todo(context: &Context, id: String) -> FieldResult<bool> {
        let mut store = context.store.lock();
        let orig_len = store.len();
        store.retain(|t| t.id != id);
        Ok(store.len() != orig_len)
    }
}

type Schema = RootNode<'static, QueryRoot, MutationRoot, EmptySubscription<Context>>;

async fn graphiql() -> Html<String> {
    Html(graphiql_source("/graphql", None))
}

async fn graphql_handler(
    Extension(schema): Extension<Arc<Schema>>,
    Extension(context): Extension<Context>,
    Json(req): Json<GraphQLRequest>,
) -> Json<juniper::http::GraphQLResponse> {
    let res = req.execute(&schema, &context).await;
    Json(res)
}

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let initial = vec![Todo {
        id: Uuid::new_v4().to_string(),
        title: "Buy milk".into(),
        completed: false,
    }];

    let store = Arc::new(Mutex::new(initial));
    let ctx = Context { store };

    let schema = Arc::new(Schema::new(
        QueryRoot,
        MutationRoot,
        EmptySubscription::new(),
    ));

    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/graphiql", get(graphiql))
        .layer(Extension(schema))
        .layer(Extension(ctx));

    Ok(app.into())
}

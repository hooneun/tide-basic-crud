use super::*;
use crate::Dino;
use sqlx::PgPool;
use tide::Error;

pub async fn create(dino: Dino, db_pool: PgPool) -> tide::Result<Dino> {
    let row = sqlx::query_as::<_, Dino>(
        "INSERT INTO dinos (id, name, weight, diet) VALUES
                 ($1, $2, $3, $4) returning id, name, weight, diet",
    )
    .bind(dino.id)
    .bind(dino.name)
    .bind(dino.weight)
    .bind(dino.diet)
    .fetch_one(&db_pool)
    .await
    .map_err(|e| Error::new(409, e))?;

    Ok(row)
}
pub async fn list(db_pool: PgPool) -> tide::Result<Vec<Dino>> {
    let rows = sqlx::query_as::<_, Dino>("SELECT id, name, weight, diet from dinos")
        .fetch_all(&db_pool)
        .await
        .map_err(|e| Error::new(409, e))?;

    Ok(rows)
}

pub async fn get(id: Uuid, db_pool: PgPool) -> tide::Result<Option<Dino>> {
    let row = sqlx::query_as::<_, Dino>("SELECT id, name, weight, diet from dinos WHERE id = $1")
        .bind(id)
        .fetch_optional(&db_pool)
        .await
        .map_err(|e| Error::new(409, e))?;

    Ok(row)
}
pub async fn delete(id: Uuid, db_pool: PgPool) -> tide::Result<Option<()>> {
    let row = sqlx::query(
        "DELETE FROM dinos
                WHERE id = $1
                returning id",
    )
    .bind(id)
    .fetch_optional(&db_pool)
    .await
    .map_err(|e| Error::new(409, e))?;

    let r = match row {
        None => None,
        Some(_) => Some(()),
    };

    Ok(r)
}

pub async fn update(id: Uuid, dino: Dino, db_pool: PgPool) -> tide::Result<Option<Dino>> {
    let row = sqlx::query_as::<_, Dino>(
        "UPDATE dinos SET name = $2, weight = $3, diet = $4
                WHERE id = $1
                returning id, name, weight, diet
                ",
    )
    .bind(id)
    .bind(dino.name)
    .bind(dino.weight)
    .bind(dino.diet)
    .fetch_optional(&db_pool)
    .await
    .map_err(|e| Error::new(409, e))?;

    Ok(row)
}

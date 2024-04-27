use anyhow::Result;
use aws_sdk_dynamodb::{
    Client,
    types::{
        AttributeDefinition, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection,
        ProjectionType, ProvisionedThroughput, ScalarAttributeType,
    },
};

pub mod ledger_entry_repository;

pub async fn delete_database(client: &Client) -> Result<()> {
    let _ = client.delete_table().table_name("a_ledger").send().await?;
    tracing::info!("a_ledger table dropped!");

    Ok(())
}

pub async fn create_database(client: &Client) -> Result<()> {
    client
        .create_table()
        .table_name("a_ledger")
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("pk")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("sk")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("account_id_and_date")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("created_at")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .key_type(KeyType::Hash)
                .attribute_name("pk")
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .key_type(KeyType::Range)
                .attribute_name("sk")
                .build()?,
        )
        .global_secondary_indexes(
            GlobalSecondaryIndex::builder()
                .index_name("a_ledger_created_at_idx")
                .key_schema(
                    KeySchemaElement::builder()
                        .key_type(KeyType::Hash)
                        .attribute_name("account_id_and_date")
                        .build()?,
                )
                .key_schema(
                    KeySchemaElement::builder()
                        .key_type(KeyType::Range)
                        .attribute_name("created_at")
                        .build()?,
                )
                .projection(
                    Projection::builder()
                        .projection_type(ProjectionType::All)
                        .build(),
                )
                .provisioned_throughput(
                    ProvisionedThroughput::builder()
                        .read_capacity_units(1)
                        .write_capacity_units(1)
                        .build()?,
                )
                .build()?,
        )
        .provisioned_throughput(
            ProvisionedThroughput::builder()
                .read_capacity_units(1)
                .write_capacity_units(1)
                .build()?,
        )
        .send()
        .await?;
    tracing::info!("a_ledger table created!");
    Ok(())
}

import { DynamoDBClient } from "@aws-sdk/client-dynamodb";
import { DynamoDBDocumentClient, GetCommand, PutCommand } from "@aws-sdk/lib-dynamodb";
import type { APIGatewayProxyEventV2, APIGatewayProxyResultV2 } from "aws-lambda";

const db = DynamoDBDocumentClient.from(new DynamoDBClient({}));
const TableName = process.env.ORDERS_TABLE!;

export async function handler(event: APIGatewayProxyEventV2): Promise<APIGatewayProxyResultV2> {
  if (event.requestContext.http.method === "POST") {
    const order = JSON.parse(event.body ?? "{}");
    if (!order.id || !order.items?.length) return { statusCode: 400, body: "invalid order" };
    await db.send(new PutCommand({ TableName, Item: order }));
    return { statusCode: 201, body: JSON.stringify(order) };
  }
  const { Item } = await db.send(new GetCommand({ TableName, Key: { id: event.pathParameters?.id } }));
  if (!Item) return { statusCode: 404, body: "not found" };
  return { statusCode: 200, body: JSON.stringify(Item) };
}

import { Stack, StackProps } from "aws-cdk-lib";
import { HttpApi, HttpMethod } from "aws-cdk-lib/aws-apigatewayv2";
import { HttpLambdaIntegration } from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { AttributeType, BillingMode, Table } from "aws-cdk-lib/aws-dynamodb";
import { Code, Function, Runtime } from "aws-cdk-lib/aws-lambda";
import { Construct } from "constructs";

export class OrdersStack extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);
    const table = new Table(this, "Orders", {
      partitionKey: { name: "id", type: AttributeType.STRING },
      billingMode: BillingMode.PAY_PER_REQUEST,
    });
    const fn = new Function(this, "OrdersFn", {
      runtime: Runtime.NODEJS_22_X,
      handler: "index.handler",
      code: Code.fromAsset("dist"),
      environment: { ORDERS_TABLE: table.tableName },
    });
    table.grant(fn, "dynamodb:PutItem", "dynamodb:GetItem");
    const api = new HttpApi(this, "OrdersApi");
    const integration = new HttpLambdaIntegration("OrdersInt", fn);
    api.addRoutes({ path: "/orders", methods: [HttpMethod.POST], integration });
    api.addRoutes({ path: "/orders/{id}", methods: [HttpMethod.GET], integration });
  }
}

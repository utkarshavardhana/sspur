import json
import os

import boto3

table = boto3.resource("dynamodb").Table(os.environ["ORDERS_TABLE"])


def handler(event, context):
    method = event["requestContext"]["http"]["method"]
    if method == "POST":
        order = json.loads(event["body"])
        if not order.get("id") or not order.get("items"):
            return {"statusCode": 400, "body": "invalid order"}
        table.put_item(Item=order)
        return {"statusCode": 201, "body": json.dumps(order)}
    order_id = event["pathParameters"]["id"]
    item = table.get_item(Key={"id": order_id}).get("Item")
    if item is None:
        return {"statusCode": 404, "body": "not found"}
    return {"statusCode": 200, "body": json.dumps(item, default=str)}


from aws_cdk import Stack, aws_apigatewayv2 as apigw, aws_apigatewayv2_integrations as integ, aws_dynamodb as ddb, aws_lambda as lambda_
from constructs import Construct


class OrdersStack(Stack):
    def __init__(self, scope: Construct, id: str, **kwargs):
        super().__init__(scope, id, **kwargs)
        table = ddb.Table(
            self, "Orders",
            partition_key=ddb.Attribute(name="id", type=ddb.AttributeType.STRING),
            billing_mode=ddb.BillingMode.PAY_PER_REQUEST,
        )
        fn = lambda_.Function(
            self, "OrdersFn",
            runtime=lambda_.Runtime.PYTHON_3_12,
            handler="app.handler",
            code=lambda_.Code.from_asset("src"),
            environment={"ORDERS_TABLE": table.table_name},
        )
        table.grant(fn, "dynamodb:PutItem", "dynamodb:GetItem")
        api = apigw.HttpApi(self, "OrdersApi")
        integration = integ.HttpLambdaIntegration("OrdersInt", fn)
        api.add_routes(path="/orders", methods=[apigw.HttpMethod.POST], integration=integration)
        api.add_routes(path="/orders/{id}", methods=[apigw.HttpMethod.GET], integration=integration)

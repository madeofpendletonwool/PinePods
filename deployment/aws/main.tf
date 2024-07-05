provider "aws" {
  region = "us-east-1" # Choose your preferred region
}

variable "db_username" {
  description = "Database administrator username"
  type        = string
  sensitive   = true
}

variable "db_password" {
  description = "Database administrator password"
  type        = string
  sensitive   = true
}

resource "aws_vpc" "main" {
  cidr_block = "10.0.0.0/16"
}

resource "aws_subnet" "subnet" {
  vpc_id     = aws_vpc.main.id
  cidr_block = "10.15.0.0/24"
}

resource "aws_security_group" "allow_all" {
  vpc_id = aws_vpc.main.id

  ingress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_rds_instance" "default" {
  allocated_storage    = 20
  engine               = "postgres"
  engine_version       = "12.5"
  instance_class       = "db.t2.micro"
  name                 = "pinepods-db"
  username             = var.db_username
  password             = var.db_password
  parameter_group_name = "default.postgres12"
  skip_final_snapshot  = true
  publicly_accessible  = true
  vpc_security_group_ids = [aws_security_group.allow_all.id]
  db_subnet_group_name   = aws_db_subnet_group.main.name
}

resource "aws_db_subnet_group" "main" {
  name       = "main"
  subnet_ids = [aws_subnet.subnet.id]

  tags = {
    Name = "Main subnet group"
  }
}

resource "aws_ecs_cluster" "main" {
  name = "pinepods-cluster"
}

resource "aws_ecs_task_definition" "pinepods" {
  family                   = "pinepods-task"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = "1"
  memory                   = "4"
  execution_role_arn       = aws_iam_role.ecs_task_execution_role.arn
  container_definitions    = <<DEFINITION
[
  {
    "name": "pinepods",
    "image": "madeofpendletonwool/pinepods", # Change this to your Docker image
    "essential": true,
    "portMappings": [
      {
        "containerPort": 80,
        "hostPort": 80
      }
    ],
    "environment": [
      {
        "name": "DB_HOST",
        "value": "${aws_rds_instance.default.address}"
      },
      {
        "name": "DB_USER",
        "value": "admin"
      },
      {
        "name": "DB_PASSWORD",
        "value": "password" # Use the same password set for RDS
      },
      {
        "name": "DB_NAME",
        "value": "pinepods"
      }
    ]
  }
]
DEFINITION
}

resource "aws_ecs_service" "main" {
  name            = "pinepods-service"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.pinepods.arn
  desired_count   = 1
  launch_type     = "FARGATE"
  network_configuration {
    subnets          = [aws_subnet.subnet.id]
    security_groups  = [aws_security_group.allow_all.id]
    assign_public_ip = true
  }
}

resource "aws_iam_role" "ecs_task_execution_role" {
  name = "ecs_task_execution_role"

  assume_role_policy = <<POLICY
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": "sts:AssumeRole",
      "Principal": {
        "Service": "ecs-tasks.amazonaws.com"
      },
      "Effect": "Allow",
      "Sid": ""
    }
  ]
}
POLICY

  managed_policy_arns = [
    "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
  ]
}
